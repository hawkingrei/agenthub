use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use crate::api::error::ApiError;
use crate::auth::UserRecord;
use crate::object_upload::{
    ObjectUploadKind, ObjectUploadOwnerScope, ObjectUploadRequest,
    ObjectUploadSessionMultipartCompletedPart, ObjectUploadSessionPrepareRequest,
};
use crate::state::AppState;

const MAX_INLINE_UPLOAD_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_UPLOAD_SESSION_TTL_SECONDS: i64 = 15 * 60;
const MAX_UPLOAD_SESSION_TTL_SECONDS: i64 = 24 * 60 * 60;

#[derive(Debug, Clone, Deserialize)]
pub struct UploadRequest {
    pub file_name: String,
    pub content_type: String,
    pub bytes_base64: String,
    pub expected_size_bytes: Option<u64>,
    pub expected_sha256: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploadSessionPrepareRequest {
    pub file_name: String,
    pub content_type: String,
    pub object_kind: String,
    pub expected_size_bytes: u64,
    pub expected_sha256: Option<String>,
    pub ttl_seconds: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploadSessionDirectWriteRequest {
    pub expires_in_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UploadSessionDirectWriteResponse {
    pub session_id: String,
    pub object_key: String,
    pub method: String,
    pub url: String,
    pub headers: Vec<UploadSessionDirectWriteHeader>,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UploadSessionDirectWriteHeader {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UploadSessionMultipartUploadResponse {
    pub session_id: String,
    pub object_key: String,
    pub upload_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploadSessionMultipartPartWriteRequest {
    pub upload_id: String,
    pub expires_in_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UploadSessionMultipartPartWriteResponse {
    pub session_id: String,
    pub object_key: String,
    pub upload_id: String,
    pub part_number: u32,
    pub method: String,
    pub url: String,
    pub headers: Vec<UploadSessionDirectWriteHeader>,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploadSessionMultipartCompleteRequest {
    pub upload_id: String,
    pub parts: Vec<UploadSessionMultipartCompletedPartRequest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploadSessionMultipartCompletedPartRequest {
    pub part_number: u32,
    pub etag: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploadSessionMultipartAbortRequest {
    pub upload_id: String,
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

pub(crate) async fn prepare_scoped_upload_session(
    State(state): State<AppState>,
    user: &UserRecord,
    owner_scope: ObjectUploadOwnerScope,
    payload: UploadSessionPrepareRequest,
) -> Result<Json<agenthub_db::ObjectUploadSessionRecord>, ApiError> {
    let content_type = normalize_upload_content_type(&payload.content_type)?;
    let kind = parse_upload_kind(&payload.object_kind)?;
    let ttl_seconds = normalize_session_ttl(payload.ttl_seconds)?;
    let session = state
        .object_uploads
        .prepare_upload_session(ObjectUploadSessionPrepareRequest {
            actor_id: canonical_user_actor_id(user),
            owner_scope,
            file_name: payload.file_name,
            content_type,
            kind,
            expected_size_bytes: payload.expected_size_bytes,
            expected_sha256: payload.expected_sha256,
            ttl_seconds,
        })
        .await
        .map_err(map_upload_error)?;
    Ok(Json(session))
}

pub(crate) async fn cancel_scoped_upload_session(
    State(state): State<AppState>,
    owner_scope: ObjectUploadOwnerScope,
    session_id: String,
) -> Result<Json<agenthub_db::ObjectUploadSessionRecord>, ApiError> {
    let session = state
        .object_uploads
        .cancel_upload_session_for_scope(owner_scope, &session_id)
        .await
        .map_err(map_upload_error)?;
    Ok(Json(session))
}

pub(crate) async fn complete_scoped_upload_session(
    State(state): State<AppState>,
    owner_scope: ObjectUploadOwnerScope,
    session_id: String,
    bytes: Bytes,
) -> Result<Json<agenthub_db::ObjectUploadRecord>, ApiError> {
    let upload = state
        .object_uploads
        .complete_upload_session_for_scope(owner_scope, &session_id, bytes.to_vec())
        .await
        .map_err(map_upload_error)?;
    Ok(Json(upload))
}

pub(crate) async fn upload_scoped_upload_session_part(
    State(state): State<AppState>,
    owner_scope: ObjectUploadOwnerScope,
    session_id: String,
    part_number: u32,
    bytes: Bytes,
) -> Result<Json<agenthub_db::ObjectUploadSessionPartRecord>, ApiError> {
    let part = state
        .object_uploads
        .upload_session_part_for_scope(owner_scope, &session_id, part_number, bytes.to_vec())
        .await
        .map_err(map_upload_error)?;
    Ok(Json(part))
}

pub(crate) async fn prepare_scoped_direct_upload_session_write(
    State(state): State<AppState>,
    owner_scope: ObjectUploadOwnerScope,
    session_id: String,
    payload: UploadSessionDirectWriteRequest,
) -> Result<Json<UploadSessionDirectWriteResponse>, ApiError> {
    let direct_write = state
        .object_uploads
        .prepare_direct_upload_session_write_for_scope(
            owner_scope,
            &session_id,
            payload.expires_in_seconds,
        )
        .await
        .map_err(map_upload_error)?;
    Ok(Json(UploadSessionDirectWriteResponse {
        session_id: direct_write.session_id,
        object_key: direct_write.object_key,
        method: direct_write.method,
        url: direct_write.uri,
        headers: direct_write
            .headers
            .into_iter()
            .map(|(name, value)| UploadSessionDirectWriteHeader { name, value })
            .collect(),
        expires_at: direct_write.expires_at,
    }))
}

pub(crate) async fn initiate_scoped_multipart_upload_session(
    State(state): State<AppState>,
    owner_scope: ObjectUploadOwnerScope,
    session_id: String,
) -> Result<Json<UploadSessionMultipartUploadResponse>, ApiError> {
    let upload = state
        .object_uploads
        .initiate_multipart_upload_session_for_scope(owner_scope, &session_id)
        .await
        .map_err(map_upload_error)?;
    Ok(Json(UploadSessionMultipartUploadResponse {
        session_id: upload.session_id,
        object_key: upload.object_key,
        upload_id: upload.upload_id,
    }))
}

pub(crate) async fn prepare_scoped_multipart_upload_session_part(
    State(state): State<AppState>,
    owner_scope: ObjectUploadOwnerScope,
    session_id: String,
    part_number: u32,
    payload: UploadSessionMultipartPartWriteRequest,
) -> Result<Json<UploadSessionMultipartPartWriteResponse>, ApiError> {
    let part = state
        .object_uploads
        .prepare_multipart_upload_session_part_for_scope(
            owner_scope,
            &session_id,
            &payload.upload_id,
            part_number,
            payload.expires_in_seconds,
        )
        .await
        .map_err(map_upload_error)?;
    Ok(Json(UploadSessionMultipartPartWriteResponse {
        session_id: part.session_id,
        object_key: part.object_key,
        upload_id: part.upload_id,
        part_number: part.part_number,
        method: part.method,
        url: part.uri,
        headers: part
            .headers
            .into_iter()
            .map(|(name, value)| UploadSessionDirectWriteHeader { name, value })
            .collect(),
        expires_at: part.expires_at,
    }))
}

pub(crate) async fn complete_scoped_upload_session_parts(
    State(state): State<AppState>,
    owner_scope: ObjectUploadOwnerScope,
    session_id: String,
) -> Result<Json<agenthub_db::ObjectUploadRecord>, ApiError> {
    let upload = state
        .object_uploads
        .complete_upload_session_parts_for_scope(owner_scope, &session_id)
        .await
        .map_err(map_upload_error)?;
    Ok(Json(upload))
}

pub(crate) async fn complete_scoped_multipart_upload_session(
    State(state): State<AppState>,
    owner_scope: ObjectUploadOwnerScope,
    session_id: String,
    payload: UploadSessionMultipartCompleteRequest,
) -> Result<Json<agenthub_db::ObjectUploadRecord>, ApiError> {
    let parts = payload
        .parts
        .into_iter()
        .map(|part| ObjectUploadSessionMultipartCompletedPart {
            part_number: part.part_number,
            etag: part.etag,
        })
        .collect();
    let upload = state
        .object_uploads
        .complete_multipart_upload_session_for_scope(
            owner_scope,
            &session_id,
            &payload.upload_id,
            parts,
        )
        .await
        .map_err(map_upload_error)?;
    Ok(Json(upload))
}

pub(crate) async fn abort_scoped_multipart_upload_session(
    State(state): State<AppState>,
    owner_scope: ObjectUploadOwnerScope,
    session_id: String,
    payload: UploadSessionMultipartAbortRequest,
) -> Result<Json<agenthub_db::ObjectUploadSessionRecord>, ApiError> {
    let session = state
        .object_uploads
        .abort_multipart_upload_session_for_scope(owner_scope, &session_id, &payload.upload_id)
        .await
        .map_err(map_upload_error)?;
    Ok(Json(session))
}

pub(crate) async fn complete_scoped_direct_upload_session(
    State(state): State<AppState>,
    owner_scope: ObjectUploadOwnerScope,
    session_id: String,
) -> Result<Json<agenthub_db::ObjectUploadRecord>, ApiError> {
    let upload = state
        .object_uploads
        .complete_direct_upload_session_for_scope(owner_scope, &session_id)
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

fn parse_upload_kind(value: &str) -> Result<ObjectUploadKind, ApiError> {
    match value.trim() {
        "object" => Ok(ObjectUploadKind::Object),
        "image" => Ok(ObjectUploadKind::Image),
        _ => Err(ApiError::bad_request(
            "object_kind must be either object or image",
        )),
    }
}

fn normalize_session_ttl(value: Option<i64>) -> Result<i64, ApiError> {
    let ttl_seconds = value.unwrap_or(DEFAULT_UPLOAD_SESSION_TTL_SECONDS);
    if !(1..=MAX_UPLOAD_SESSION_TTL_SECONDS).contains(&ttl_seconds) {
        return Err(ApiError::bad_request(&format!(
            "ttl_seconds must be between 1 and {MAX_UPLOAD_SESSION_TTL_SECONDS}"
        )));
    }
    Ok(ttl_seconds)
}

fn decode_upload_bytes(value: &str) -> Result<Vec<u8>, ApiError> {
    let bytes = STANDARD
        .decode(value.trim())
        .map_err(|_| ApiError::bad_request("bytes_base64 must be valid standard base64"))?;
    if bytes.len() > MAX_INLINE_UPLOAD_BYTES {
        return Err(ApiError::payload_too_large(&format!(
            "inline uploads are limited to {MAX_INLINE_UPLOAD_BYTES} bytes"
        )));
    }
    Ok(bytes)
}

fn map_upload_error(err: anyhow::Error) -> ApiError {
    let message = err.to_string();
    if message.contains("file_name")
        || message.contains("size mismatch")
        || message.contains("sha256")
        || message.contains("expected_size_bytes")
        || message.contains("ttl_seconds")
        || message.contains("part_number")
        || message.contains("uploaded parts")
        || message.contains("contiguous")
        || message.contains("upload_id")
        || message.contains("multipart upload")
        || message.contains("s3 multipart uploads")
        || message.contains("expires_in_seconds")
        || message.contains("presigned object writes")
        || message.contains("unsupported hosted image content type")
        || message.contains("expired")
    {
        return ApiError::bad_request(&message);
    }
    if message.contains("does not belong") || message.contains("not cancelable") {
        return ApiError::conflict(&message);
    }
    err.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    #[test]
    fn decode_upload_bytes_rejects_payloads_above_inline_limit() {
        let encoded = STANDARD.encode(vec![0_u8; MAX_INLINE_UPLOAD_BYTES + 1]);
        let err = decode_upload_bytes(&encoded).expect_err("oversized inline payload should fail");
        assert_eq!(
            err.into_response().status(),
            axum::http::StatusCode::PAYLOAD_TOO_LARGE
        );
    }
}
