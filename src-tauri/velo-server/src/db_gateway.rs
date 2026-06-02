//! SQL gateway — mirrors the Tauri SQL plugin's `select`/`execute` contract over
//! HTTP so the frontend's `connection.ts` works unchanged on the web.
//!
//! Per-user routing: each request runs against the *logged-in user's own*
//! data DB (`data-{userId}.db`), resolved from the `User` placed in request
//! extensions by the `require_user` middleware. Isolation is by file boundary —
//! a member's SQL can only ever touch their own mailbox data.
//!
//! Single-connection model per user DB: all SQL for a given user is serialized
//! on ONE connection behind a mutex. This matches the desktop (the Tauri SQL
//! plugin + the frontend's `withTransaction` mutex assume a single connection)
//! and makes multi-request transactions correct — `BEGIN`, the statements, and
//! `COMMIT` arrive as separate HTTP calls but all run on the same connection.
//!
//! Placeholders are `$1, $2, …` exactly as the frontend already emits.

use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Extension, Router,
};
use serde::Deserialize;
use serde_json::{json, Map, Value as JsonValue};
use sqlx::{Column, Row, TypeInfo, ValueRef};

use crate::state::{AppState, Conn, User};

#[derive(Deserialize)]
struct SqlReq {
    query: String,
    #[serde(default)]
    params: Vec<JsonValue>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/select", post(select))
        .route("/execute", post(execute))
        .with_state(state)
}

/// Build a sqlx query and bind JSON params positionally, mirroring how the
/// Tauri SQL plugin maps JS values onto sqlite bindings.
fn bind_params(
    query: &str,
    params: Vec<JsonValue>,
) -> sqlx::query::Query<'_, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'_>> {
    let mut q = sqlx::query(query);
    for p in params {
        q = match p {
            JsonValue::Null => q.bind(None::<String>),
            JsonValue::Bool(b) => q.bind(b),
            JsonValue::Number(n) => {
                if let Some(i) = n.as_i64() {
                    q.bind(i)
                } else if let Some(f) = n.as_f64() {
                    q.bind(f)
                } else {
                    q.bind(n.to_string())
                }
            }
            JsonValue::String(s) => q.bind(s),
            other => q.bind(other.to_string()),
        };
    }
    q
}

fn err_response(msg: String) -> Response {
    // Log gateway errors server-side so they appear in the host's logs, not only
    // in the HTTP response body the browser receives.
    tracing::warn!("db gateway error: {msg}");
    (StatusCode::BAD_GATEWAY, Json(json!({ "error": msg }))).into_response()
}

async fn select(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Json(req): Json<SqlReq>,
) -> Response {
    let conn: Conn = state.user_db(&user.id).await;
    let mut conn = conn.lock().await;
    let q = bind_params(&req.query, req.params);
    match q.fetch_all(&mut *conn).await {
        Ok(rows) => {
            let out: Vec<JsonValue> = rows.iter().map(row_to_json).collect();
            Json(out).into_response()
        }
        Err(e) => {
            let snippet: String = req.query.chars().take(120).collect();
            err_response(format!("select failed: {e} | query: {snippet}"))
        }
    }
}

async fn execute(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Json(req): Json<SqlReq>,
) -> Response {
    let conn: Conn = state.user_db(&user.id).await;
    let mut conn = conn.lock().await;

    // Recover from a connection left mid-transaction. The frontend runs
    // BEGIN / ... / COMMIT as separate HTTP calls on one shared connection; if a
    // page was closed/refreshed between BEGIN and COMMIT, the connection stays
    // stuck and every later BEGIN fails with "cannot start a transaction within
    // a transaction". Before a BEGIN, roll back any dangling transaction so the
    // new one can start cleanly.
    let trimmed = req.query.trim_start();
    if trimmed.len() >= 5 && trimmed[..5].eq_ignore_ascii_case("BEGIN") {
        use sqlx::Executor;
        let _ = conn.execute("ROLLBACK").await; // no-op if not in a transaction
    }

    let q = bind_params(&req.query, req.params);
    match q.execute(&mut *conn).await {
        Ok(result) => Json(json!({
            "rowsAffected": result.rows_affected(),
            "lastInsertId": result.last_insert_rowid(),
        }))
        .into_response(),
        Err(e) => {
            let snippet: String = req.query.chars().take(120).collect();
            err_response(format!("execute failed: {e} | query: {snippet}"))
        }
    }
}

/// Convert a SQLite row into a JSON object keyed by column name, matching the
/// shape the frontend expects from the Tauri SQL plugin's `select`.
fn row_to_json(row: &sqlx::sqlite::SqliteRow) -> JsonValue {
    let mut map = Map::new();
    for col in row.columns() {
        let i = col.ordinal();
        map.insert(col.name().to_string(), value_to_json(row, i));
    }
    JsonValue::Object(map)
}

fn value_to_json(row: &sqlx::sqlite::SqliteRow, i: usize) -> JsonValue {
    let raw = match row.try_get_raw(i) {
        Ok(r) => r,
        Err(_) => return JsonValue::Null,
    };
    if raw.is_null() {
        return JsonValue::Null;
    }

    let type_name = raw.type_info().name().to_uppercase();
    match type_name.as_str() {
        "TEXT" => row
            .try_get::<String, _>(i)
            .map(JsonValue::from)
            .unwrap_or(JsonValue::Null),
        "INTEGER" | "BIGINT" | "INT" | "INT8" => row
            .try_get::<i64, _>(i)
            .map(JsonValue::from)
            .unwrap_or(JsonValue::Null),
        "REAL" | "DOUBLE" | "FLOAT" | "NUMERIC" => row
            .try_get::<f64, _>(i)
            .map(JsonValue::from)
            .unwrap_or(JsonValue::Null),
        "BOOLEAN" => row
            .try_get::<bool, _>(i)
            .map(JsonValue::from)
            .unwrap_or(JsonValue::Null),
        "BLOB" => row
            .try_get::<Vec<u8>, _>(i)
            .map(|bytes| JsonValue::Array(bytes.into_iter().map(JsonValue::from).collect()))
            .unwrap_or(JsonValue::Null),
        _ => {
            if let Ok(n) = row.try_get::<i64, _>(i) {
                JsonValue::from(n)
            } else if let Ok(f) = row.try_get::<f64, _>(i) {
                JsonValue::from(f)
            } else if let Ok(s) = row.try_get::<String, _>(i) {
                JsonValue::from(s)
            } else {
                JsonValue::Null
            }
        }
    }
}
