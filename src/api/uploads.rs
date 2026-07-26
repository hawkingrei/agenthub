use axum::Json;
use axum::extract::State;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Deserialize;

use crate::api::error::ApiError;
use crate::auth::UserRecord;
use crate::object_upload::{
    ObjectDownloadRequest, ObjectUploadKind, ObjectUploadOwnerScope, ObjectUploadRequest,
};
use crate::state::AppState;

#[derive(Debug, Clone, Deserialize)]
pub struct UploadRequest {
    pub file_name: String,
    pub content_type: String,
    pub bytes_base64: String,
    pub expected_size_bytes: Option<u64>,
    pub expected_sha256: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadRequest {
    pub source_url: String,
    pub file_name: String,
    pub content_type: String,
    pub expected_size_bytes: Option<u64>,
    pub expected_sha256: Option<String>,
}

pub(crate) fn canonical_user_actor_id(user: &UserRecord) -> String {
    format!("user:{}", user.id)
}

pub(crate) async fn upload_scoped_object(
    State(state): State<AppState>,
    user: &UserRecord,
    owner_scope: ObjectUploadOwnerScope,
    payload: UploadRequest,
    kind: ObjectUploadKind,
) -> Result<Json<agenthub_db::ObjectUploadRecord>, ApiError> {
    let content_type = normalize_upload_content_type(&payload.content_type)?;
    let bytes = decode_upload_bytes(&payload.bytes_base64)?;
    let upload = state
        .object_uploads
        .upload(ObjectUploadRequest {
            actor_id: canonical_user_actor_id(user),
            owner_scope,
            file_name: payload.file_name,
            content_type,
            kind,
            expected_size_bytes: payload.expected_size_bytes,
            expected_sha256: payload.expected_sha256,
            bytes,
        })
        .await
        .map_err(map_upload_error)?;
    Ok(Json(upload))
}

pub(crate) async fn download_scoped_object(
    State(state): State<AppState>,
    user: &UserRecord,
    owner_scope: ObjectUploadOwnerScope,
    payload: DownloadRequest,
) -> Result<Json<agenthub_db::ObjectUploadRecord>, ApiError> {
    let content_type = normalize_upload_content_type(&payload.content_type)?;
    let upload = state
        .object_uploads
        .download(ObjectDownloadRequest {
            actor_id: canonical_user_actor_id(user),
            owner_scope,
            file_name: payload.file_name,
            content_type,
            source_url: payload.source_url,
            expected_size_bytes: payload.expected_size_bytes,
            expected_sha256: payload.expected_sha256,
        })
        .await
        .map_err(map_upload_error)?;
    Ok(Json(upload))
}

fn normalize_upload_content_type(value: &str) -> Result<String, ApiError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ApiError::bad_request("content_type is required"));
    }
    Ok(value.to_ascii_lowercase())
}

fn decode_upload_bytes(value: &str) -> Result<Vec<u8>, ApiError> {
    STANDARD
        .decode(value.trim())
        .map_err(|_| ApiError::bad_request("bytes_base64 must be valid standard base64"))
}

fn map_upload_error(err: anyhow::Error) -> ApiError {
    let message = err.to_string();
    if message.contains("file_name")
        || message.contains("size mismatch")
        || message.contains("sha256")
        || message.contains("unsupported hosted image content type")
        || message.contains("source_url")
        || message.contains("download")
    {
        return ApiError::bad_request(&message);
    }
    err.into()
}
