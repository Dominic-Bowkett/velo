//! velo-server — self-hosted web backend for Velo.
//!
//! Serves the built frontend and the JSON API the browser talks to. All mail is
//! IMAP/SMTP. Access is per-user: an admin creates users and (later) assigns
//! mailboxes; each user's mailbox data lives in its own SQLite file, so members
//! can only ever see their own mail.
//!
//! Run with:  cargo run -p velo-server
//! Env:
//!   VELO_BIND           bind address              (default 127.0.0.1:8080)
//!   VELO_CONTROL_DB     control DB (users/sessions) (default ./velo-control.db)
//!   VELO_DATA_DIR       per-user data DB directory  (default ./data)
//!   VELO_STATIC_DIR     built frontend dir          (optional)
//!   VELO_ADMIN_EMAIL    bootstrap admin email       (first run only)
//!   VELO_ADMIN_PASSWORD bootstrap admin password    (first run only)

mod auth;
mod db_gateway;
mod email_api;
mod oauth_api;
mod state;

use axum::{routing::get, Router};
use state::AppState;
use std::net::SocketAddr;
use std::path::PathBuf;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "velo_server=info,tower_http=info".into()),
        )
        .init();

    let control_db =
        std::env::var("VELO_CONTROL_DB").unwrap_or_else(|_| "velo-control.db".to_string());
    let data_dir = PathBuf::from(std::env::var("VELO_DATA_DIR").unwrap_or_else(|_| "data".into()));
    std::fs::create_dir_all(&data_dir).expect("failed to create VELO_DATA_DIR");

    let state = AppState::init(&control_db, data_dir).await;
    let app = build_app(state);

    let bind = std::env::var("VELO_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let addr: SocketAddr = bind.parse().expect("VELO_BIND must be a valid address");

    tracing::info!("velo-server listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");
    axum::serve(listener, app).await.expect("server error");
}

/// Build the full application router. Separated from `main` so tests can drive
/// it without binding a socket.
fn build_app(state: AppState) -> Router {
    // Protected API: email commands + SQL gateway require a signed-in user.
    let protected = Router::new()
        .merge(email_api::router())
        .nest("/oauth", oauth_api::router())
        .nest("/db", db_gateway::router(state.clone()))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::require_user,
        ));

    // Public/self-scoped API: health + auth (login/logout/me) + admin user mgmt
    // (admin routes are guarded inside auth::router).
    let api = Router::new()
        .route("/health", get(|| async { "ok" }))
        .merge(auth::router(state.clone()))
        .merge(protected);

    let mut app = Router::new().nest("/api", api).layer(CorsLayer::permissive());

    // Optionally serve the built frontend (single-page app) from VELO_STATIC_DIR.
    // Unknown paths fall back to index.html so client-side routes (e.g. /settings)
    // load the SPA instead of 404ing.
    if let Ok(dir) = std::env::var("VELO_STATIC_DIR") {
        let index = format!("{dir}/index.html");
        app = app.fallback_service(ServeDir::new(dir).not_found_service(ServeFile::new(index)));
    }

    app
}

#[cfg(test)]
mod tests {
    use super::build_app;
    use crate::state::AppState;
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use serde_json::{json, Value};
    use std::path::PathBuf;
    use tower::ServiceExt;

    /// Build an app backed by temp control + data DBs and a bootstrapped admin.
    async fn test_app() -> (axum::Router, PathBuf) {
        let dir = std::env::temp_dir().join(format!("velo-test-{}", crate::state::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("VELO_ADMIN_EMAIL", "admin@example.com");
        std::env::set_var("VELO_ADMIN_PASSWORD", "admin-pass-123");
        let control = dir.join("control.db");
        let state = AppState::init(control.to_str().unwrap(), dir.clone()).await;
        (build_app(state), dir)
    }

    async fn send(app: &axum::Router, req: Request<Body>) -> (StatusCode, Value, Vec<String>) {
        let resp = app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let cookies: Vec<String> = resp
            .headers()
            .get_all("set-cookie")
            .iter()
            .map(|v| v.to_str().unwrap().to_string())
            .collect();
        let bytes = to_bytes(resp.into_body(), 1 << 20).await.unwrap();
        let val: Value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::Null)
        };
        (status, val, cookies)
    }

    fn json_post(uri: &str, body: Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    async fn login(app: &axum::Router, email: &str, password: &str) -> String {
        let (status, _, cookies) =
            send(app, json_post("/api/auth/login", json!({ "email": email, "password": password })))
                .await;
        assert_eq!(status, StatusCode::OK, "login should succeed");
        // Extract the session cookie value (name=value before the first ';').
        cookies
            .iter()
            .find(|c| c.starts_with("velo_session="))
            .map(|c| c.split(';').next().unwrap().to_string())
            .expect("session cookie set")
    }

    #[tokio::test]
    async fn health_is_public() {
        let (app, _d) = test_app().await;
        let (status, _, _) = send(
            &app,
            Request::builder().uri("/api/health").body(Body::empty()).unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn db_gateway_requires_auth() {
        let (app, _d) = test_app().await;
        let (status, _, _) = send(
            &app,
            json_post("/api/db/select", json!({ "query": "SELECT 1", "params": [] })),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn admin_can_login_and_fetch_me() {
        let (app, _d) = test_app().await;
        let cookie = login(&app, "admin@example.com", "admin-pass-123").await;
        let (status, body, _) = send(
            &app,
            Request::builder()
                .uri("/api/auth/me")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["role"], "admin");
        assert_eq!(body["email"], "admin@example.com");
    }

    #[tokio::test]
    async fn bad_password_rejected() {
        let (app, _d) = test_app().await;
        let (status, _, _) = send(
            &app,
            json_post("/api/auth/login", json!({ "email": "admin@example.com", "password": "wrong" })),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn member_cannot_use_admin_routes() {
        let (app, _d) = test_app().await;
        let admin = login(&app, "admin@example.com", "admin-pass-123").await;

        // Admin creates a member.
        let (status, body, _) = send(
            &app,
            {
                let mut r = json_post(
                    "/api/admin/users",
                    json!({ "email": "mark@example.com", "password": "mark-pass-123", "role": "member" }),
                );
                r.headers_mut().insert("cookie", admin.parse().unwrap());
                r
            },
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["role"], "member");

        // Member logs in and is forbidden from listing users.
        let member = login(&app, "mark@example.com", "mark-pass-123").await;
        let (status, _, _) = send(
            &app,
            Request::builder()
                .uri("/api/admin/users")
                .header("cookie", &member)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn each_user_has_isolated_data_db() {
        let (app, _d) = test_app().await;
        let admin = login(&app, "admin@example.com", "admin-pass-123").await;
        send(
            &app,
            {
                let mut r = json_post(
                    "/api/admin/users",
                    json!({ "email": "mark@example.com", "password": "mark-pass-123" }),
                );
                r.headers_mut().insert("cookie", admin.parse().unwrap());
                r
            },
        )
        .await;
        let member = login(&app, "mark@example.com", "mark-pass-123").await;

        // Admin writes into their own data DB.
        let mut create = json_post(
            "/api/db/execute",
            json!({ "query": "CREATE TABLE t (v TEXT)", "params": [] }),
        );
        create.headers_mut().insert("cookie", admin.parse().unwrap());
        send(&app, create).await;
        let mut ins = json_post(
            "/api/db/execute",
            json!({ "query": "INSERT INTO t (v) VALUES ('admin-secret')", "params": [] }),
        );
        ins.headers_mut().insert("cookie", admin.parse().unwrap());
        send(&app, ins).await;

        // The member's data DB does not have that table — proves file isolation.
        let mut sel = json_post(
            "/api/db/select",
            json!({ "query": "SELECT v FROM t", "params": [] }),
        );
        sel.headers_mut().insert("cookie", member.parse().unwrap());
        let (status, body, _) = send(&app, sel).await;
        assert_eq!(status, StatusCode::BAD_GATEWAY); // "no such table: t" in member DB
        assert!(body["error"].as_str().unwrap().contains("no such table"));
    }

    #[tokio::test]
    async fn sql_gateway_fts5_trigram_available() {
        let (app, _d) = test_app().await;
        let admin = login(&app, "admin@example.com", "admin-pass-123").await;
        let mut r = json_post(
            "/api/db/execute",
            json!({ "query": "CREATE VIRTUAL TABLE fts USING fts5(body, tokenize='trigram')", "params": [] }),
        );
        r.headers_mut().insert("cookie", admin.parse().unwrap());
        let (status, _, _) = send(&app, r).await;
        assert_eq!(status, StatusCode::OK, "FTS5 + trigram must be available");
    }
}
