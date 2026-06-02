//! velo-server — self-hosted web backend for Velo.
//!
//! Phase 1 scope: HTTP mirror of the desktop IMAP/SMTP/OAuth commands, plus
//! static serving of the built frontend. Later phases add the SQL gateway,
//! per-user auth/roles, the OAuth redirect callback, and background workers.
//!
//! Run with:  cargo run -p velo-server
//! Env:
//!   VELO_BIND        bind address           (default 127.0.0.1:8080)
//!   VELO_STATIC_DIR  built frontend dir      (default ./dist; optional)

mod db_gateway;
mod email_api;
mod oauth_api;

use axum::{routing::get, Router};
use db_gateway::DbState;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
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

    let db = open_db().await;
    let app = build_app(db);

    let bind = std::env::var("VELO_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let addr: SocketAddr = bind.parse().expect("VELO_BIND must be a valid address");

    tracing::info!("velo-server listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");
    axum::serve(listener, app).await.expect("server error");
}

/// Open (creating if needed) the single SQLite connection backing the SQL
/// gateway. Path comes from VELO_DB (default ./velo.db). The frontend runs its
/// own migrations through the gateway, so the server just needs a live handle.
async fn open_db() -> DbState {
    use sqlx::sqlite::SqliteConnectOptions;
    use sqlx::ConnectOptions;

    let path = std::env::var("VELO_DB").unwrap_or_else(|_| "velo.db".to_string());
    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{path}"))
        .expect("invalid VELO_DB path")
        .create_if_missing(true)
        // WAL improves concurrent read performance for the single-writer model.
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

    let conn = opts.connect().await.expect("failed to open SQLite database");
    tracing::info!("SQLite database ready at {path}");
    Arc::new(Mutex::new(conn))
}

/// Build the full application router. Separated from `main` so it can be
/// exercised by tests without binding a socket.
fn build_app(db: DbState) -> Router {
    // /api/* — the JSON API the frontend's httpTransport talks to.
    let api = Router::new()
        .route("/health", get(|| async { "ok" }))
        .merge(email_api::router())
        .nest("/oauth", oauth_api::router())
        .nest("/db", db_gateway::router(db));

    let mut app = Router::new().nest("/api", api).layer(CorsLayer::permissive());

    // Optionally serve the built frontend (single-page app) from VELO_STATIC_DIR.
    // Unknown paths fall back to index.html so client-side routes (e.g. /settings,
    // /help/...) load the SPA instead of 404ing.
    if let Ok(dir) = std::env::var("VELO_STATIC_DIR") {
        let index = format!("{dir}/index.html");
        app = app.fallback_service(
            ServeDir::new(dir).not_found_service(ServeFile::new(index)),
        );
    }

    app
}

#[cfg(test)]
mod tests {
    use super::build_app;
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use serde_json::{json, Value};
    use sqlx::Connection;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tower::ServiceExt; // for `oneshot`

    async fn test_app() -> axum::Router {
        // In-memory SQLite so each test is isolated and needs no file.
        let conn = sqlx::SqliteConnection::connect("sqlite::memory:")
            .await
            .unwrap();
        build_app(Arc::new(Mutex::new(conn)))
    }

    async fn post(app: &axum::Router, uri: &str, body: Value) -> (StatusCode, Value) {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = resp.status();
        let bytes = to_bytes(resp.into_body(), 1 << 20).await.unwrap();
        let val: Value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::Null)
        };
        (status, val)
    }

    #[tokio::test]
    async fn health_endpoint_responds_ok() {
        let app = test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn unknown_route_is_404() {
        let app = test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/does-not-exist")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn sql_gateway_roundtrip() {
        let app = test_app().await;

        // DDL via execute
        let (s, _) = post(
            &app,
            "/api/db/execute",
            json!({ "query": "CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT, n INTEGER)", "params": [] }),
        )
        .await;
        assert_eq!(s, StatusCode::OK);

        // INSERT with positional params; check rowsAffected + lastInsertId
        let (s, body) = post(
            &app,
            "/api/db/execute",
            json!({ "query": "INSERT INTO t (name, n) VALUES ($1, $2)", "params": ["alice", 42] }),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(body["rowsAffected"], 1);
        assert_eq!(body["lastInsertId"], 1);

        // SELECT returns array of objects keyed by column, with correct types
        let (s, rows) = post(
            &app,
            "/api/db/select",
            json!({ "query": "SELECT id, name, n FROM t WHERE name = $1", "params": ["alice"] }),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert!(rows.is_array());
        assert_eq!(rows[0]["id"], 1);
        assert_eq!(rows[0]["name"], "alice");
        assert_eq!(rows[0]["n"], 42);
    }

    #[tokio::test]
    async fn sql_gateway_fts5_available() {
        // The desktop schema relies on FTS5; verify the gateway's SQLite has it.
        let app = test_app().await;
        let (s, _) = post(
            &app,
            "/api/db/execute",
            json!({ "query": "CREATE VIRTUAL TABLE fts USING fts5(body)", "params": [] }),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "FTS5 must be compiled into SQLite");
    }

    #[tokio::test]
    async fn sql_gateway_fts5_trigram_tokenizer() {
        // migrations.ts creates messages_fts with tokenize='trigram'.
        let app = test_app().await;
        let (s, _) = post(
            &app,
            "/api/db/execute",
            json!({ "query": "CREATE VIRTUAL TABLE fts USING fts5(body, tokenize='trigram')", "params": [] }),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "trigram tokenizer must be supported");

        post(&app, "/api/db/execute", json!({ "query": "INSERT INTO fts (body) VALUES ('hello world')", "params": [] })).await;
        let (_, rows) = post(
            &app,
            "/api/db/select",
            json!({ "query": "SELECT body FROM fts WHERE fts MATCH $1", "params": ["wor"] }),
        )
        .await;
        assert_eq!(rows[0]["body"], "hello world");
    }

    #[tokio::test]
    async fn sql_gateway_multi_request_transaction() {
        // BEGIN / INSERT / COMMIT arrive as separate calls but must share the
        // one connection — exactly what withTransaction relies on.
        let app = test_app().await;
        post(&app, "/api/db/execute", json!({ "query": "CREATE TABLE t (id INTEGER)", "params": [] })).await;

        let (s, _) = post(&app, "/api/db/execute", json!({ "query": "BEGIN", "params": [] })).await;
        assert_eq!(s, StatusCode::OK);
        post(&app, "/api/db/execute", json!({ "query": "INSERT INTO t (id) VALUES (1)", "params": [] })).await;
        let (s, _) = post(&app, "/api/db/execute", json!({ "query": "COMMIT", "params": [] })).await;
        assert_eq!(s, StatusCode::OK);

        let (_, rows) = post(&app, "/api/db/select", json!({ "query": "SELECT id FROM t", "params": [] })).await;
        assert_eq!(rows[0]["id"], 1);
    }
}
