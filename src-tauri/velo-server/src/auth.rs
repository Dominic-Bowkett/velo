//! Authentication + session management and admin user management.
//!
//! - `POST /api/auth/login`  { email, password } → sets a `velo_session` cookie
//! - `POST /api/auth/logout`                     → clears the session
//! - `GET  /api/auth/me`                         → current user (role, email)
//! - Admin-only user management under `/api/admin/users`
//!
//! `current_user` is the middleware extractor: it reads the session cookie,
//! looks up the user in the control DB, and rejects unauthenticated requests to
//! protected routes with 401.

use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::state::{
    hash_password, new_id, new_token, verify_password, AppState, User,
};

pub const SESSION_COOKIE: &str = "velo_session";

/// Auth + admin routes. The auth routes are public (login) or self-scoped
/// (logout/me); the admin routes are guarded by `require_admin`.
pub fn router(state: AppState) -> Router {
    let admin = Router::new()
        .route("/users", get(list_users).post(create_user))
        .route("/users/:id", axum::routing::delete(delete_user))
        .route("/users/:id/password", post(set_user_password))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_admin,
        ));

    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
        .nest("/admin", admin)
        .with_state(state)
}

// ---------- session lookup ----------

/// Resolve the logged-in user from the session cookie, or None.
pub async fn user_from_jar(state: &AppState, jar: &CookieJar) -> Option<User> {
    let token = jar.get(SESSION_COOKIE)?.value().to_string();
    let mut control = state.control.lock().await;
    let row: Option<User> = sqlx::query_as(
        "SELECT u.id, u.email, u.display_name, u.role, u.password_hash \
         FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.token = $1",
    )
    .bind(&token)
    .fetch_optional(&mut *control)
    .await
    .ok()
    .flatten();
    row
}

/// Middleware: require any authenticated user. Inserts `User` into request
/// extensions for downstream handlers.
pub async fn require_user(
    State(state): State<AppState>,
    jar: CookieJar,
    mut req: axum::extract::Request,
    next: Next,
) -> Response {
    match user_from_jar(&state, &jar).await {
        Some(user) => {
            req.extensions_mut().insert(user);
            next.run(req).await
        }
        None => (StatusCode::UNAUTHORIZED, Json(json!({ "error": "Not signed in" })))
            .into_response(),
    }
}

/// Middleware: require an admin user.
pub async fn require_admin(
    State(state): State<AppState>,
    jar: CookieJar,
    mut req: axum::extract::Request,
    next: Next,
) -> Response {
    match user_from_jar(&state, &jar).await {
        Some(user) if user.is_admin() => {
            req.extensions_mut().insert(user);
            next.run(req).await
        }
        Some(_) => (StatusCode::FORBIDDEN, Json(json!({ "error": "Admin only" })))
            .into_response(),
        None => (StatusCode::UNAUTHORIZED, Json(json!({ "error": "Not signed in" })))
            .into_response(),
    }
}

// ---------- login / logout / me ----------

#[derive(Deserialize)]
struct LoginReq {
    email: String,
    password: String,
}

#[derive(Serialize)]
struct UserView {
    id: String,
    email: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    role: String,
}

impl From<&User> for UserView {
    fn from(u: &User) -> Self {
        UserView {
            id: u.id.clone(),
            email: u.email.clone(),
            display_name: u.display_name.clone(),
            role: u.role.clone(),
        }
    }
}

fn session_cookie(token: String) -> Cookie<'static> {
    let mut c = Cookie::new(SESSION_COOKIE, token);
    c.set_http_only(true);
    c.set_same_site(SameSite::Lax);
    c.set_path("/");
    // `Secure` is omitted so it works over plain HTTP on a LAN; put the server
    // behind TLS in production (a reverse proxy) for cookie confidentiality.
    c
}

async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(req): Json<LoginReq>,
) -> Response {
    let mut control = state.control.lock().await;
    let user: Option<User> = sqlx::query_as(
        "SELECT id, email, display_name, role, password_hash FROM users WHERE email = $1",
    )
    .bind(&req.email)
    .fetch_optional(&mut *control)
    .await
    .ok()
    .flatten();

    let user = match user {
        Some(u) if verify_password(&req.password, &u.password_hash) => u,
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Invalid email or password" })),
            )
                .into_response()
        }
    };

    let token = new_token();
    sqlx::query("INSERT INTO sessions (token, user_id) VALUES ($1, $2)")
        .bind(&token)
        .bind(&user.id)
        .execute(&mut *control)
        .await
        .expect("failed to create session");
    drop(control);

    let jar = jar.add(session_cookie(token));
    (jar, Json(UserView::from(&user))).into_response()
}

async fn logout(State(state): State<AppState>, jar: CookieJar) -> Response {
    if let Some(c) = jar.get(SESSION_COOKIE) {
        let token = c.value().to_string();
        let mut control = state.control.lock().await;
        let _ = sqlx::query("DELETE FROM sessions WHERE token = $1")
            .bind(&token)
            .execute(&mut *control)
            .await;
    }
    let jar = jar.remove(Cookie::from(SESSION_COOKIE));
    (jar, Json(json!({ "ok": true }))).into_response()
}

async fn me(State(state): State<AppState>, jar: CookieJar) -> Response {
    match user_from_jar(&state, &jar).await {
        Some(u) => Json(UserView::from(&u)).into_response(),
        None => (StatusCode::UNAUTHORIZED, Json(json!({ "error": "Not signed in" })))
            .into_response(),
    }
}

// ---------- admin user management ----------

#[derive(Deserialize)]
struct CreateUserReq {
    email: String,
    password: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    role: Option<String>, // defaults to "member"
}

async fn list_users(State(state): State<AppState>) -> Response {
    let mut control = state.control.lock().await;
    let users: Vec<User> =
        sqlx::query_as("SELECT id, email, display_name, role, password_hash FROM users ORDER BY email")
            .fetch_all(&mut *control)
            .await
            .unwrap_or_default();
    let views: Vec<UserView> = users.iter().map(UserView::from).collect();
    Json(views).into_response()
}

async fn create_user(
    State(state): State<AppState>,
    Json(req): Json<CreateUserReq>,
) -> Response {
    let role = req.role.unwrap_or_else(|| "member".to_string());
    if role != "admin" && role != "member" {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "Invalid role" })))
            .into_response();
    }
    if req.password.len() < 8 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Password must be at least 8 characters" })),
        )
            .into_response();
    }

    let id = new_id();
    let hash = hash_password(&req.password);
    let mut control = state.control.lock().await;
    let result = sqlx::query(
        "INSERT INTO users (id, email, display_name, role, password_hash) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&id)
    .bind(&req.email)
    .bind(&req.display_name)
    .bind(&role)
    .bind(&hash)
    .execute(&mut *control)
    .await;

    match result {
        Ok(_) => Json(json!({
            "id": id, "email": req.email, "displayName": req.display_name, "role": role
        }))
        .into_response(),
        Err(e) => {
            let msg = if e.to_string().contains("UNIQUE") {
                "A user with that email already exists".to_string()
            } else {
                format!("Failed to create user: {e}")
            };
            (StatusCode::BAD_REQUEST, Json(json!({ "error": msg }))).into_response()
        }
    }
}

async fn delete_user(
    State(state): State<AppState>,
    axum::Extension(admin): axum::Extension<User>,
    Path(id): Path<String>,
) -> Response {
    if id == admin.id {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "You cannot delete your own account" })),
        )
            .into_response();
    }
    let mut control = state.control.lock().await;
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(&id)
        .execute(&mut *control)
        .await;
    Json(json!({ "ok": true })).into_response()
}

#[derive(Deserialize)]
struct SetPasswordReq {
    password: String,
}

async fn set_user_password(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<SetPasswordReq>,
) -> Response {
    if req.password.len() < 8 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Password must be at least 8 characters" })),
        )
            .into_response();
    }
    let hash = hash_password(&req.password);
    let mut control = state.control.lock().await;
    let _ = sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
        .bind(&hash)
        .bind(&id)
        .execute(&mut *control)
        .await;
    Json(json!({ "ok": true })).into_response()
}
