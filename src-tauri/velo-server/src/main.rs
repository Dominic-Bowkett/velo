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

mod email_api;
mod oauth_api;

use axum::{routing::get, Router};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "velo_server=info,tower_http=info".into()),
        )
        .init();

    let app = build_app();

    let bind = std::env::var("VELO_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let addr: SocketAddr = bind.parse().expect("VELO_BIND must be a valid address");

    tracing::info!("velo-server listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");
    axum::serve(listener, app).await.expect("server error");
}

/// Build the full application router. Separated from `main` so it can be
/// exercised by tests without binding a socket.
fn build_app() -> Router {
    // /api/* — the JSON API the frontend's httpTransport talks to.
    let api = Router::new()
        .route("/health", get(|| async { "ok" }))
        .merge(email_api::router())
        .nest("/oauth", oauth_api::router());

    let mut app = Router::new().nest("/api", api).layer(CorsLayer::permissive());

    // Optionally serve the built frontend (single-page app) from VELO_STATIC_DIR.
    if let Ok(dir) = std::env::var("VELO_STATIC_DIR") {
        app = app.fallback_service(ServeDir::new(dir));
    }

    app
}

#[cfg(test)]
mod tests {
    use super::build_app;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt; // for `oneshot`

    #[tokio::test]
    async fn health_endpoint_responds_ok() {
        let app = build_app();
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
        let app = build_app();
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
}
