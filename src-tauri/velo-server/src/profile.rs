//! Admin-controlled profile settings: outgoing display name and signature.
//!
//! Settings are stored per scope in the control DB. Scope is either the literal
//! `"global"` or a specific user id. Resolution for a user is: per-user value if
//! set, else the global value, else empty. Members cannot change these — only
//! the admin endpoints write them; members read their resolved profile.
//!
//!   GET  /api/profile                      → resolved { displayName, signatureHtml } for caller
//!   GET  /api/admin/profile                → { global, perUser: [...] } (admin)
//!   PUT  /api/admin/profile/global         → set global display name / signature
//!   PUT  /api/admin/profile/user/:id       → set per-user display name / signature

use axum::{
    extract::{Json, Path, State},
    response::{IntoResponse, Response},
    routing::{get, put},
    Extension, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::state::{AppState, User};

pub const GLOBAL_SCOPE: &str = "global";

pub async fn migrate(conn: &mut sqlx::SqliteConnection) {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS profile_settings (
            scope TEXT PRIMARY KEY,          -- 'global' or a user id
            display_name TEXT,
            signature_html TEXT
        )",
    )
    .execute(&mut *conn)
    .await
    .expect("create profile_settings table failed");
}

pub fn member_router(state: AppState) -> Router {
    Router::new()
        .route("/profile", get(get_my_profile))
        .with_state(state)
}

pub fn admin_router(state: AppState) -> Router {
    Router::new()
        .route("/profile", get(get_all))
        .route("/profile/global", put(set_global))
        .route("/profile/user/:id", put(set_user))
        .with_state(state)
}

#[derive(Default, Serialize)]
struct ProfileValue {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "signatureHtml")]
    signature_html: Option<String>,
}

async fn fetch_scope(state: &AppState, scope: &str) -> ProfileValue {
    let mut control = state.control.lock().await;
    let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT display_name, signature_html FROM profile_settings WHERE scope = $1",
    )
    .bind(scope)
    .fetch_optional(&mut *control)
    .await
    .ok()
    .flatten();
    match row {
        Some((dn, sig)) => ProfileValue {
            display_name: dn,
            signature_html: sig,
        },
        None => ProfileValue::default(),
    }
}

/// Resolved profile for a user: per-user value overrides global, field by field.
async fn get_my_profile(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
) -> Response {
    let global = fetch_scope(&state, GLOBAL_SCOPE).await;
    let mine = fetch_scope(&state, &user.id).await;
    let resolved = ProfileValue {
        display_name: mine.display_name.or(global.display_name),
        signature_html: mine.signature_html.or(global.signature_html),
    };
    Json(resolved).into_response()
}

#[derive(Serialize)]
struct PerUserProfile {
    #[serde(rename = "userId")]
    user_id: String,
    email: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "signatureHtml")]
    signature_html: Option<String>,
}

/// Admin view: the global setting plus each user's explicit (unresolved) values.
async fn get_all(State(state): State<AppState>) -> Response {
    let global = fetch_scope(&state, GLOBAL_SCOPE).await;
    let mut control = state.control.lock().await;
    let rows: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT u.id, u.email, p.display_name, p.signature_html \
         FROM users u LEFT JOIN profile_settings p ON p.scope = u.id \
         ORDER BY u.email",
    )
    .fetch_all(&mut *control)
    .await
    .unwrap_or_default();
    let per_user: Vec<PerUserProfile> = rows
        .into_iter()
        .map(|(user_id, email, dn, sig)| PerUserProfile {
            user_id,
            email,
            display_name: dn,
            signature_html: sig,
        })
        .collect();
    Json(json!({ "global": global, "perUser": per_user })).into_response()
}

#[derive(Deserialize)]
struct SetProfile {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "signatureHtml")]
    signature_html: Option<String>,
}

async fn upsert_scope(state: &AppState, scope: &str, body: SetProfile) -> Response {
    let mut control = state.control.lock().await;
    let result = sqlx::query(
        "INSERT INTO profile_settings (scope, display_name, signature_html) \
         VALUES ($1, $2, $3) \
         ON CONFLICT(scope) DO UPDATE SET display_name = $2, signature_html = $3",
    )
    .bind(scope)
    .bind(&body.display_name)
    .bind(&body.signature_html)
    .execute(&mut *control)
    .await;
    match result {
        Ok(_) => Json(json!({ "ok": true })).into_response(),
        Err(e) => (
            axum::http::StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Failed to save profile: {e}") })),
        )
            .into_response(),
    }
}

async fn set_global(State(state): State<AppState>, Json(body): Json<SetProfile>) -> Response {
    upsert_scope(&state, GLOBAL_SCOPE, body).await
}

async fn set_user(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<SetProfile>,
) -> Response {
    upsert_scope(&state, &id, body).await
}
