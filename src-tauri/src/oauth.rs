//! Tauri command wrappers for OAuth. The actual logic lives in
//! `velo_core::oauth`, shared with the web server (which serves the callback
//! over HTTP instead of a localhost socket).

use velo_core::oauth;
pub use velo_core::oauth::{OAuthResult, TokenExchangeResult};

/// Binds to a localhost port for the OAuth callback (desktop PKCE flow).
#[tauri::command]
pub async fn start_oauth_server(port: u16, state: String) -> Result<OAuthResult, String> {
    oauth::start_oauth_server(port, state).await
}

/// Exchange an OAuth authorization code for tokens (avoids browser CORS).
#[tauri::command]
pub async fn oauth_exchange_token(
    token_url: String,
    code: String,
    client_id: String,
    redirect_uri: String,
    code_verifier: Option<String>,
    client_secret: Option<String>,
    scope: Option<String>,
) -> Result<TokenExchangeResult, String> {
    oauth::oauth_exchange_token(
        token_url,
        code,
        client_id,
        redirect_uri,
        code_verifier,
        client_secret,
        scope,
    )
    .await
}

/// Refresh an OAuth token (avoids browser CORS).
#[tauri::command]
pub async fn oauth_refresh_token(
    token_url: String,
    refresh_token: String,
    client_id: String,
    client_secret: Option<String>,
    scope: Option<String>,
) -> Result<TokenExchangeResult, String> {
    oauth::oauth_refresh_token(token_url, refresh_token, client_id, client_secret, scope).await
}
