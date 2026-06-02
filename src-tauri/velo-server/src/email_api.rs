//! HTTP handlers mirroring the desktop Tauri IMAP/SMTP/OAuth commands.
//!
//! Each route accepts a JSON body matching the parameters of the corresponding
//! `velo_core::ops` function and returns its result as JSON. Errors from core
//! (`Result<_, String>`) are surfaced as HTTP 502 with a JSON `{ "error": ... }`
//! body so the frontend's `httpTransport` can treat them like the rejected
//! promises that `invoke()` produced on the desktop.
//!
//! NOTE: in this phase the IMAP/SMTP `config` (which carries credentials) is
//! still supplied in the request body, mirroring the desktop contract. Phase 3
//! moves credential resolution server-side (by accountId) so the browser never
//! sends secrets; these handlers will then look the config up instead.

use axum::{
    extract::Json,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use velo_core::ops;
use velo_core::{
    DeltaCheckRequest, ImapConfig, SmtpConfig,
};

/// Convert a `Result<T, String>` from core into an HTTP response: `T` as JSON on
/// success, or 502 + `{ "error": msg }` on failure.
fn core_result<T: Serialize>(result: Result<T, String>) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(msg) => (StatusCode::BAD_GATEWAY, Json(json!({ "error": msg }))).into_response(),
    }
}

pub fn router() -> Router {
    Router::new()
        .route("/imap/test_connection", post(imap_test_connection))
        .route("/imap/list_folders", post(imap_list_folders))
        .route("/imap/fetch_messages", post(imap_fetch_messages))
        .route("/imap/fetch_new_uids", post(imap_fetch_new_uids))
        .route("/imap/search_all_uids", post(imap_search_all_uids))
        .route("/imap/fetch_message_body", post(imap_fetch_message_body))
        .route("/imap/fetch_raw_message", post(imap_fetch_raw_message))
        .route("/imap/set_flags", post(imap_set_flags))
        .route("/imap/move_messages", post(imap_move_messages))
        .route("/imap/delete_messages", post(imap_delete_messages))
        .route("/imap/get_folder_status", post(imap_get_folder_status))
        .route("/imap/fetch_attachment", post(imap_fetch_attachment))
        .route("/imap/append_message", post(imap_append_message))
        .route("/imap/search_folder", post(imap_search_folder))
        .route("/imap/sync_folder", post(imap_sync_folder))
        .route("/imap/raw_fetch_diagnostic", post(imap_raw_fetch_diagnostic))
        .route("/imap/delta_check", post(imap_delta_check))
        .route("/smtp/send", post(smtp_send_email))
        .route("/smtp/test_connection", post(smtp_test_connection))
}

// ---------- IMAP request bodies ----------

#[derive(Deserialize)]
struct ConfigOnly {
    config: ImapConfig,
}

#[derive(Deserialize)]
struct FolderReq {
    config: ImapConfig,
    folder: String,
}

#[derive(Deserialize)]
struct FetchMessagesReq {
    config: ImapConfig,
    folder: String,
    uids: Vec<u32>,
}

#[derive(Deserialize)]
struct NewUidsReq {
    config: ImapConfig,
    folder: String,
    #[serde(rename = "sinceUid")]
    since_uid: u32,
}

#[derive(Deserialize)]
struct MessageBodyReq {
    config: ImapConfig,
    folder: String,
    uid: u32,
}

#[derive(Deserialize)]
struct SetFlagsReq {
    config: ImapConfig,
    folder: String,
    uids: Vec<u32>,
    flags: Vec<String>,
    add: bool,
}

#[derive(Deserialize)]
struct MoveReq {
    config: ImapConfig,
    folder: String,
    uids: Vec<u32>,
    destination: String,
}

#[derive(Deserialize)]
struct DeleteReq {
    config: ImapConfig,
    folder: String,
    uids: Vec<u32>,
}

#[derive(Deserialize)]
struct AttachmentReq {
    config: ImapConfig,
    folder: String,
    uid: u32,
    #[serde(rename = "partId")]
    part_id: String,
}

#[derive(Deserialize)]
struct AppendReq {
    config: ImapConfig,
    folder: String,
    flags: Option<String>,
    #[serde(rename = "rawMessage")]
    raw_message: String,
}

#[derive(Deserialize)]
struct SearchFolderReq {
    config: ImapConfig,
    folder: String,
    #[serde(rename = "sinceDate")]
    since_date: Option<String>,
}

#[derive(Deserialize)]
struct SyncFolderReq {
    config: ImapConfig,
    folder: String,
    #[serde(rename = "batchSize")]
    batch_size: u32,
    #[serde(rename = "sinceDate")]
    since_date: Option<String>,
}

#[derive(Deserialize)]
struct RawDiagnosticReq {
    config: ImapConfig,
    folder: String,
    #[serde(rename = "uidRange")]
    uid_range: String,
}

#[derive(Deserialize)]
struct DeltaCheckReq {
    config: ImapConfig,
    folders: Vec<DeltaCheckRequest>,
}

// ---------- SMTP request bodies ----------

#[derive(Deserialize)]
struct SmtpSendReq {
    config: SmtpConfig,
    #[serde(rename = "rawEmail")]
    raw_email: String,
}

#[derive(Deserialize)]
struct SmtpConfigOnly {
    config: SmtpConfig,
}

// ---------- IMAP handlers ----------

async fn imap_test_connection(Json(req): Json<ConfigOnly>) -> Response {
    core_result(ops::imap_test_connection(req.config).await)
}

async fn imap_list_folders(Json(req): Json<ConfigOnly>) -> Response {
    core_result(ops::imap_list_folders(req.config).await)
}

async fn imap_fetch_messages(Json(req): Json<FetchMessagesReq>) -> Response {
    core_result(ops::imap_fetch_messages(req.config, req.folder, req.uids).await)
}

async fn imap_fetch_new_uids(Json(req): Json<NewUidsReq>) -> Response {
    core_result(ops::imap_fetch_new_uids(req.config, req.folder, req.since_uid).await)
}

async fn imap_search_all_uids(Json(req): Json<FolderReq>) -> Response {
    core_result(ops::imap_search_all_uids(req.config, req.folder).await)
}

async fn imap_fetch_message_body(Json(req): Json<MessageBodyReq>) -> Response {
    core_result(ops::imap_fetch_message_body(req.config, req.folder, req.uid).await)
}

async fn imap_fetch_raw_message(Json(req): Json<MessageBodyReq>) -> Response {
    core_result(ops::imap_fetch_raw_message(req.config, req.folder, req.uid).await)
}

async fn imap_set_flags(Json(req): Json<SetFlagsReq>) -> Response {
    core_result(ops::imap_set_flags(req.config, req.folder, req.uids, req.flags, req.add).await)
}

async fn imap_move_messages(Json(req): Json<MoveReq>) -> Response {
    core_result(ops::imap_move_messages(req.config, req.folder, req.uids, req.destination).await)
}

async fn imap_delete_messages(Json(req): Json<DeleteReq>) -> Response {
    core_result(ops::imap_delete_messages(req.config, req.folder, req.uids).await)
}

async fn imap_get_folder_status(Json(req): Json<FolderReq>) -> Response {
    core_result(ops::imap_get_folder_status(req.config, req.folder).await)
}

async fn imap_fetch_attachment(Json(req): Json<AttachmentReq>) -> Response {
    core_result(ops::imap_fetch_attachment(req.config, req.folder, req.uid, req.part_id).await)
}

async fn imap_append_message(Json(req): Json<AppendReq>) -> Response {
    core_result(ops::imap_append_message(req.config, req.folder, req.flags, req.raw_message).await)
}

async fn imap_search_folder(Json(req): Json<SearchFolderReq>) -> Response {
    core_result(ops::imap_search_folder(req.config, req.folder, req.since_date).await)
}

async fn imap_sync_folder(Json(req): Json<SyncFolderReq>) -> Response {
    core_result(
        ops::imap_sync_folder(req.config, req.folder, req.batch_size, req.since_date).await,
    )
}

async fn imap_raw_fetch_diagnostic(Json(req): Json<RawDiagnosticReq>) -> Response {
    core_result(ops::imap_raw_fetch_diagnostic(req.config, req.folder, req.uid_range).await)
}

async fn imap_delta_check(Json(req): Json<DeltaCheckReq>) -> Response {
    core_result(ops::imap_delta_check(req.config, req.folders).await)
}

// ---------- SMTP handlers ----------

async fn smtp_send_email(Json(req): Json<SmtpSendReq>) -> Response {
    core_result(ops::smtp_send_email(req.config, req.raw_email).await)
}

async fn smtp_test_connection(Json(req): Json<SmtpConfigOnly>) -> Response {
    core_result(ops::smtp_test_connection(req.config).await)
}
