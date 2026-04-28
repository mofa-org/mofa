//! File storage endpoints backed by the kernel `ObjectStore` trait
//!
//! POST   /api/v1/files/upload              - upload via multipart/form-data
//! GET    /api/v1/files/:key               - download file bytes
//! DELETE /api/v1/files/:key               - delete a file
//! GET    /api/v1/files                     - list keys (optional ?prefix= query)
//! GET    /api/v1/files/:key/presigned-get - generate a presigned GET URL
//! POST   /api/v1/files/:key/presigned-put - generate a presigned PUT URL
//! GET    /api/v1/files/:key/metadata      - return content-type, size, last-modified
//!
//! All handlers return `501 Not Implemented` when no object store has been
//! configured on the server (i.e. `AppState::s3` is `None`).

use axum::{
    Json,
    body::Bytes,
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::error::GatewayError;
use crate::state::AppState;

// ─────────────────────────────────────────────────────────────────────────────
// DTOs
// ─────────────────────────────────────────────────────────────────────────────

/// Query parameters for GET /api/v1/files
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    /// Only return keys that begin with this prefix.
    #[serde(default)]
    pub prefix: String,
}

/// Request body for POST /api/v1/files/{key}/presigned-put
#[derive(Debug, Deserialize)]
pub struct PresignedPutRequest {
    /// Seconds until the presigned URL expires (default: 3600).
    #[serde(default = "default_expires")]
    pub expires_secs: u64,
    /// Optional MIME type that the client must include in the `Content-Type`
    /// header when uploading via the returned URL.
    pub content_type: Option<String>,
}

fn default_expires() -> u64 {
    3600
}

/// Query parameters for GET /api/v1/files/{key}/presigned-get
#[derive(Debug, Deserialize)]
pub struct PresignedGetQuery {
    /// Seconds until the presigned URL expires (default: 3600).
    #[serde(default = "default_expires")]
    pub expires: u64,
}

/// Response returned after a successful upload.
#[derive(Debug, Serialize)]
pub struct UploadResponse {
    /// Object key under which the file was stored.
    pub key: String,
    /// Number of bytes written.
    pub size: usize,
    /// Detected or supplied MIME type.
    pub content_type: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// MIME helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Guess the MIME type from a file key's extension.
///
/// Falls back to `"application/octet-stream"` when the extension is unknown.
#[cfg(feature = "s3")]
fn guess_mime(key: &str) -> &'static str {
    mime_guess::from_path(key)
        .first_raw()
        .unwrap_or("application/octet-stream")
}

#[cfg(not(feature = "s3"))]
fn guess_mime(_key: &str) -> &'static str {
    "application/octet-stream"
}

// ─────────────────────────────────────────────────────────────────────────────
// Socket.IO progress helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Emit a file-upload lifecycle event to all Socket.IO clients on the namespace.
///
/// Does nothing when Socket.IO is not configured or the `socketio` feature is
/// disabled.
fn emit_upload_event(state: &AppState, event: &str, key: &str, extra: serde_json::Value) {
    #[cfg(feature = "socketio")]
    if let Some((io, ns)) = &state.socketio {
        if let Some(namespace) = io.of(ns.as_str()) {
            let mut payload = json!({ "key": key });
            if let (Some(obj), Some(ext)) = (payload.as_object_mut(), extra.as_object()) {
                for (k, v) in ext {
                    obj.insert(k.clone(), v.clone());
                }
            }
            let _ = namespace.emit(event, &payload);
        }
    }
    // suppress unused-variable warnings when socketio feature is off
    let _ = (state, event, key, extra);
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper
// ─────────────────────────────────────────────────────────────────────────────

/// Return a reference to the object store or a `501 Not Implemented` error.
macro_rules! store {
    ($state:expr) => {
        $state
            .s3
            .as_deref()
            .ok_or(GatewayError::NotConfigured("S3 object store"))?
    };
}

// ─────────────────────────────────────────────────────────────────────────────
// Handlers
// ─────────────────────────────────────────────────────────────────────────────

/// POST /api/v1/files/upload
///
/// Accepts `multipart/form-data` with two fields:
/// - `key`  — destination object key (e.g. `"uploads/photo.jpg"`)
/// - `file` — raw file bytes
///
/// Enforces `AppState::max_upload_bytes` if set.
/// Emits Socket.IO events `file_upload_started` / `file_upload_completed` /
/// `file_upload_failed` when the Socket.IO bridge is configured.
pub async fn upload_file(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, GatewayError> {
    let s3 = store!(state);

    let mut key: Option<String> = None;
    let mut data: Option<Vec<u8>> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| GatewayError::InvalidRequest(format!("multipart error: {}", e)))?
    {
        match field.name() {
            Some("key") => {
                key =
                    Some(field.text().await.map_err(|e| {
                        GatewayError::InvalidRequest(format!("key read error: {}", e))
                    })?);
            }
            Some("file") => {
                data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| {
                            GatewayError::InvalidRequest(format!("file read error: {}", e))
                        })?
                        .to_vec(),
                );
            }
            _ => {}
        }
    }

    let key = key.ok_or_else(|| GatewayError::InvalidRequest("missing 'key' field".into()))?;
    let data = data.ok_or_else(|| GatewayError::InvalidRequest("missing 'file' field".into()))?;

    if key.is_empty() {
        return Err(GatewayError::InvalidRequest("key must not be empty".into()));
    }

    // File size limit
    if let Some(max) = state.max_upload_bytes {
        if data.len() as u64 > max {
            let msg = format!(
                "file size {} bytes exceeds the configured limit of {} bytes",
                data.len(),
                max
            );
            emit_upload_event(
                &state,
                "file_upload_failed",
                &key,
                json!({ "reason": &msg }),
            );
            return Err(GatewayError::PayloadTooLarge(msg));
        }
    }

    // Auto-detect MIME type from key extension
    let content_type = guess_mime(&key);

    emit_upload_event(
        &state,
        "file_upload_started",
        &key,
        json!({ "size": data.len(), "content_type": content_type }),
    );

    let size = data.len();
    if let Err(e) = s3.put(&key, data).await {
        let msg = e.to_string();
        emit_upload_event(
            &state,
            "file_upload_failed",
            &key,
            json!({ "reason": &msg }),
        );
        return Err(GatewayError::S3Error(msg));
    }

    tracing::info!(key = %key, bytes = size, content_type, "file uploaded");

    emit_upload_event(
        &state,
        "file_upload_completed",
        &key,
        json!({ "size": size, "content_type": content_type }),
    );

    Ok((
        StatusCode::CREATED,
        Json(UploadResponse {
            key,
            size,
            content_type: Some(content_type.to_string()),
        }),
    ))
}

/// GET /api/v1/files/{key}
///
/// Downloads the object stored at `key` and streams its bytes back.
/// Returns `404 Not Found` if the key does not exist.
/// Sets `Content-Type` based on the file extension.
pub async fn download_file(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> Result<impl IntoResponse, GatewayError> {
    let s3 = store!(state);

    let bytes = s3
        .get(&key)
        .await
        .map_err(|e| GatewayError::S3Error(e.to_string()))?
        .ok_or_else(|| GatewayError::AgentNotFound(format!("file not found: {}", key)))?;

    let mime = guess_mime(&key);
    let content_type = HeaderValue::from_str(mime)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, content_type);

    Ok((StatusCode::OK, headers, Bytes::from(bytes)))
}

/// DELETE /api/v1/files/{key}
///
/// Deletes the object stored at `key`.
/// Returns `404 Not Found` if the key did not exist.
pub async fn delete_file(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> Result<impl IntoResponse, GatewayError> {
    let s3 = store!(state);

    let existed = s3
        .delete(&key)
        .await
        .map_err(|e| GatewayError::S3Error(e.to_string()))?;

    if !existed {
        return Err(GatewayError::AgentNotFound(format!(
            "file not found: {}",
            key
        )));
    }

    tracing::info!(key = %key, "file deleted");
    Ok((
        StatusCode::OK,
        Json(json!({ "key": key, "status": "deleted" })),
    ))
}

/// GET /api/v1/files?prefix=...
///
/// Lists object keys. Pass `?prefix=uploads/` to filter by prefix.
pub async fn list_files(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListQuery>,
) -> Result<impl IntoResponse, GatewayError> {
    let s3 = store!(state);

    let keys = s3
        .list_keys(&params.prefix)
        .await
        .map_err(|e| GatewayError::S3Error(e.to_string()))?;

    Ok(Json(json!({
        "keys": keys,
        "total": keys.len(),
        "prefix": params.prefix,
    })))
}

/// GET /api/v1/files/{key}/presigned-get?expires=3600
///
/// Returns a time-limited presigned URL that allows anyone to `GET` the object
/// directly from the storage backend without routing bytes through this server.
pub async fn presigned_get(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
    Query(params): Query<PresignedGetQuery>,
    _headers: HeaderMap,
) -> Result<impl IntoResponse, GatewayError> {
    let s3 = store!(state);

    let url = s3
        .presigned_get_url(&key, params.expires)
        .await
        .map_err(|e| GatewayError::S3Error(e.to_string()))?;

    Ok(Json(json!({
        "key": key,
        "url": url,
        "expires_secs": params.expires,
        "method": "GET",
    })))
}

/// POST /api/v1/files/{key}/presigned-put
///
/// Returns a time-limited presigned URL that allows a client to upload an object
/// directly to the storage backend (bypassing this server).
///
/// Request body (JSON):
/// ```json
/// { "expires_secs": 3600, "content_type": "image/jpeg" }
/// ```
pub async fn presigned_put(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
    Json(req): Json<PresignedPutRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    let s3 = store!(state);

    // Auto-detect content_type from extension when not provided by caller
    let resolved_ct = req
        .content_type
        .as_deref()
        .map(|s| s.to_string())
        .or_else(|| {
            let m = guess_mime(&key);
            if m == "application/octet-stream" {
                None
            } else {
                Some(m.to_string())
            }
        });

    let url = s3
        .presigned_put_url(&key, req.expires_secs, resolved_ct.as_deref())
        .await
        .map_err(|e| GatewayError::S3Error(e.to_string()))?;

    Ok(Json(json!({
        "key": key,
        "url": url,
        "expires_secs": req.expires_secs,
        "method": "PUT",
        "content_type": resolved_ct,
    })))
}

/// GET /api/v1/files/{key}/metadata
///
/// Returns object metadata without downloading the file body.
///
/// Response (JSON):
/// ```json
/// {
///   "key": "uploads/photo.jpg",
///   "size": 204800,
///   "content_type": "image/jpeg",
///   "last_modified": "2025-01-15T12:34:56+00:00"
/// }
/// ```
///
/// Returns `404 Not Found` if the key does not exist.
/// Returns `501 Not Implemented` if the backend does not support metadata queries.
pub async fn file_metadata(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> Result<impl IntoResponse, GatewayError> {
    let s3 = store!(state);

    let meta = s3
        .get_metadata(&key)
        .await
        .map_err(|e| GatewayError::S3Error(e.to_string()))?
        .ok_or_else(|| GatewayError::AgentNotFound(format!("file not found: {}", key)))?;

    Ok(Json(json!({
        "key": key,
        "size": meta.size,
        "content_type": meta.content_type,
        "last_modified": meta.last_modified,
    })))
}

// ─────────────────────────────────────────────────────────────────────────────
// Router
// ─────────────────────────────────────────────────────────────────────────────

/// Build the files sub-router.
pub fn files_router() -> axum::Router<Arc<AppState>> {
    use axum::routing::{delete, get, post};
    axum::Router::new()
        .route("/api/v1/files/upload", post(upload_file))
        .route("/api/v1/files/:key", get(download_file).delete(delete_file))
        .route("/api/v1/files", get(list_files))
        .route("/api/v1/files/:key/presigned-get", get(presigned_get))
        .route("/api/v1/files/:key/presigned-put", post(presigned_put))
        .route("/api/v1/files/:key/metadata", get(file_metadata))
}
