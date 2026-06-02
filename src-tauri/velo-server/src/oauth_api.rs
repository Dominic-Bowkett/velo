//! HTTP handlers for OAuth token exchange/refresh (CORS proxy), mirroring the
//! desktop `oauth_exchange_token` / `oauth_refresh_token` commands.
//!
//! The desktop `start_oauth_server` (a localhost TCP listener) has no web
//! equivalent — on the web the browser is redirected to the provider and back
//! to `/api/oauth/callback`, handled in Phase 2/3. Here we expose the two
//! credential-free proxy endpoints that already work identically server-side.

use axum::{
    extract::Json,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use serde::Deserialize;
use serde_json::json;
use velo_core::oauth;

pub fn router() -> Router {
    Router::new()
        .route("/exchange_token", post(exchange_token))
        .route("/refresh_token", post(refresh_token))
}

#[derive(Deserialize)]
struct ExchangeReq {
    #[serde(rename = "tokenUrl")]
    token_url: String,
    code: String,
    #[serde(rename = "clientId")]
    client_id: String,
    #[serde(rename = "redirectUri")]
    redirect_uri: String,
    #[serde(rename = "codeVerifier")]
    code_verifier: Option<String>,
    #[serde(rename = "clientSecret")]
    client_secret: Option<String>,
    scope: Option<String>,
}

#[derive(Deserialize)]
struct RefreshReq {
    #[serde(rename = "tokenUrl")]
    token_url: String,
    #[serde(rename = "refreshToken")]
    refresh_token: String,
    #[serde(rename = "clientId")]
    client_id: String,
    #[serde(rename = "clientSecret")]
    client_secret: Option<String>,
    scope: Option<String>,
}

async fn exchange_token(Json(req): Json<ExchangeReq>) -> Response {
    match oauth::oauth_exchange_token(
        req.token_url,
        req.code,
        req.client_id,
        req.redirect_uri,
        req.code_verifier,
        req.client_secret,
        req.scope,
    )
    .await
    {
        Ok(v) => Json(v).into_response(),
        Err(msg) => (StatusCode::BAD_GATEWAY, Json(json!({ "error": msg }))).into_response(),
    }
}

async fn refresh_token(Json(req): Json<RefreshReq>) -> Response {
    match oauth::oauth_refresh_token(
        req.token_url,
        req.refresh_token,
        req.client_id,
        req.client_secret,
        req.scope,
    )
    .await
    {
        Ok(v) => Json(v).into_response(),
        Err(msg) => (StatusCode::BAD_GATEWAY, Json(json!({ "error": msg }))).into_response(),
    }
}
