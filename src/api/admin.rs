use argon2::{Argon2, password_hash::PasswordHasher};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{delete, get, post},
};
use chrono::Utc;
use rand::Rng;
use sqlx::Row;
use uuid::Uuid;

use crate::api::authz::require_root;
use crate::api::error::ApiError;
use crate::api::{extract_ip, extract_ua, ok_response};
use crate::path_utils::expand_tilde;
use crate::state::AppState;

#[derive(Debug, serde::Serialize)]
pub struct SafePath {
    pub path: String,
    pub created_at: i64,
}

#[derive(Debug, serde::Deserialize)]
pub struct AddSafePathRequest {
    pub path: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct SetPasskeyEnabledRequest {
    pub enabled: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct AdminSettingsResponse {
    pub passkey_enabled: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct DeviceRecord {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub user_agent: String,
    pub status: String,
    pub created_at: i64,
    pub last_login_at: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct JoinStartResponse {
    pub token: String,
    pub pin: String,
    pub expires_at: i64,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/safe_paths", get(list_safe_paths).post(add_safe_path))
        .route("/safe_paths", delete(delete_safe_path))
        .route("/devices", get(list_devices))
        .route("/devices/{id}/revoke", post(revoke_device))
        .route("/audits", get(list_audits))
        .route("/settings", get(get_settings))
        .route("/settings/passkey", post(set_passkey_enabled))
        .route("/join/start", post(join_start))
        .with_state(state)
}

async fn list_safe_paths(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SafePath>>, ApiError> {
    let _user = require_root(&headers, &state).await?;
    let rows = sqlx::query("SELECT path, created_at FROM safe_paths ORDER BY id ASC")
        .fetch_all(&state.db)
        .await?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(SafePath {
            path: row.get("path"),
            created_at: row.get("created_at"),
        });
    }
    Ok(Json(items))
}

async fn add_safe_path(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AddSafePathRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = require_root(&headers, &state).await?;
    let now = Utc::now().timestamp();
    let path = expand_tilde(payload.path.trim());
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO safe_paths (path, created_at)
        VALUES (?1, ?2)
        "#,
    )
    .bind(path)
    .bind(now)
    .execute(&state.db)
    .await?;
    let detail = format!("path={}", payload.path.trim());
    let _ = state
        .auth
        .record_audit(
            Some(&user.id),
            None,
            "safe_path_added",
            Some(&detail),
            extract_ip(&headers).as_deref(),
            extract_ua(&headers).as_deref(),
        )
        .await;
    Ok(ok_response())
}

async fn delete_safe_path(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AddSafePathRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = require_root(&headers, &state).await?;
    sqlx::query("DELETE FROM safe_paths WHERE path = ?1")
        .bind(expand_tilde(payload.path.trim()))
        .execute(&state.db)
        .await?;
    let detail = format!("path={}", payload.path.trim());
    let _ = state
        .auth
        .record_audit(
            Some(&user.id),
            None,
            "safe_path_deleted",
            Some(&detail),
            extract_ip(&headers).as_deref(),
            extract_ua(&headers).as_deref(),
        )
        .await;
    Ok(ok_response())
}

async fn list_devices(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<DeviceRecord>>, ApiError> {
    let _user = require_root(&headers, &state).await?;
    let rows = sqlx::query(
        r#"
        SELECT id, user_id, name, user_agent, status, created_at, last_login_at
        FROM devices
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(&state.db)
    .await?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(DeviceRecord {
            id: row.get("id"),
            user_id: row.get("user_id"),
            name: row.get("name"),
            user_agent: row.get("user_agent"),
            status: row.get("status"),
            created_at: row.get("created_at"),
            last_login_at: row.get("last_login_at"),
        });
    }
    Ok(Json(items))
}

async fn revoke_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(device_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = require_root(&headers, &state).await?;
    let now = Utc::now().timestamp();
    let row = sqlx::query(
        r#"
        SELECT user_id FROM devices WHERE id = ?1
        "#,
    )
    .bind(&device_id)
    .fetch_optional(&state.db)
    .await?;
    let user_id: Option<String> = row.map(|r| r.get("user_id"));

    sqlx::query(
        r#"
        UPDATE devices
        SET status = 'revoked'
        WHERE id = ?1
        "#,
    )
    .bind(&device_id)
    .execute(&state.db)
    .await?;

    if let Some(uid) = user_id.as_deref() {
        let _ = sqlx::query(
            r#"
            UPDATE auth_sessions
            SET revoked_at = ?1
            WHERE user_id = ?2
            "#,
        )
        .bind(now)
        .bind(uid)
        .execute(&state.db)
        .await;

        let _ = state
            .auth
            .record_audit(
                Some(&user.id),
                Some(uid),
                "device_revoked",
                Some(&format!("device_id={}", device_id)),
                extract_ip(&headers).as_deref(),
                extract_ua(&headers).as_deref(),
            )
            .await;
    }

    Ok(ok_response())
}

async fn list_audits(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    let _user = require_root(&headers, &state).await?;
    let limit = params
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(50);
    let rows = sqlx::query(
        r#"
        SELECT id, user_id, target_user_id, action, detail, ip, user_agent, created_at
        FROM audit_log
        ORDER BY created_at DESC
        LIMIT ?1
        "#,
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let ip: Option<String> = row.get("ip");
        let ua: Option<String> = row.get("user_agent");
        items.push(serde_json::json!({
            "id": row.get::<String, _>("id"),
            "user_id": row.get::<Option<String>, _>("user_id"),
            "target_user_id": row.get::<Option<String>, _>("target_user_id"),
            "action": row.get::<String, _>("action"),
            "detail": row.get::<Option<String>, _>("detail"),
            "ip": ip,
            "user_agent": ua,
            "created_at": row.get::<i64, _>("created_at"),
        }));
    }
    Ok(Json(items))
}

async fn get_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AdminSettingsResponse>, ApiError> {
    let _user = require_root(&headers, &state).await?;
    let passkey_enabled = state.auth.is_passkey_enabled().await?;
    Ok(Json(AdminSettingsResponse { passkey_enabled }))
}

async fn set_passkey_enabled(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SetPasskeyEnabledRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_root(&headers, &state).await?;
    let passkey_enabled = payload.enabled;
    state.auth.set_passkey_enabled(passkey_enabled).await?;
    Ok(ok_response())
}

async fn join_start(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<JoinStartResponse>, ApiError> {
    let _user = require_root(&headers, &state).await?;
    let pin: String = rand::rng()
        .sample_iter(&rand::distr::Alphanumeric)
        .take(6)
        .map(char::from)
        .collect();
    let pin_hash = {
        let salt = argon2::password_hash::SaltString::generate(&mut rand::rng());
        Argon2::default()
            .hash_password(pin.as_bytes(), &salt)
            .map_err(|e| ApiError::bad_request(&format!("pin hashing failed: {e}")))?
            .to_string()
    };
    let expires_at = Utc::now().timestamp() + 3600;
    sqlx::query(
        r#"
        INSERT INTO join_pins (pin_hash, created_by, expires_at)
        VALUES (?1, ?2, ?3)
        "#,
    )
    .bind(&pin_hash)
    .bind(&_user.id)
    .bind(expires_at)
    .execute(&state.db)
    .await?;

    let token = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO join_tokens (token, pin_hash, created_by, expires_at, used)
        VALUES (?1, ?2, ?3, ?4, 0)
        "#,
    )
    .bind(&token)
    .bind(&pin_hash)
    .bind(&_user.id)
    .bind(expires_at)
    .execute(&state.db)
    .await?;

    Ok(Json(JoinStartResponse {
        token,
        pin,
        expires_at,
    }))
}
