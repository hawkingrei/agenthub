use argon2::{Argon2, password_hash::PasswordHasher};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{delete, get, post},
};
use chrono::Utc;
use rand::RngCore;
use sqlx::Row;
use uuid::Uuid;

use crate::api::authz::require_user;
use crate::api::error::ApiError;
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
        .route("/devices/:id/revoke", post(revoke_device))
        .route("/audits", get(list_audits))
        .route("/join/start", post(join_start))
        .with_state(state)
}

async fn list_safe_paths(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SafePath>>, ApiError> {
    let user = require_user(&headers, &state).await?;
    if user.role != "root" {
        return Err(ApiError::unauthorized("root required"));
    }
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
    let user = require_user(&headers, &state).await?;
    if user.role != "root" {
        return Err(ApiError::unauthorized("root required"));
    }
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
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn delete_safe_path(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AddSafePathRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = require_user(&headers, &state).await?;
    if user.role != "root" {
        return Err(ApiError::unauthorized("root required"));
    }
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
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

fn expand_tilde(path: &str) -> String {
    if path == "~" {
        return std::env::var("HOME").unwrap_or_else(|_| path.to_string());
    }
    if let Some(stripped) = path.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return format!("{}/{}", home, stripped);
    }
    path.to_string()
}

fn extract_ip(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
}

fn extract_ua(headers: &HeaderMap) -> Option<String> {
    headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

async fn list_devices(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<DeviceRecord>>, ApiError> {
    let user = require_user(&headers, &state).await?;
    if user.role != "root" {
        return Err(ApiError::unauthorized("root required"));
    }
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
    let user = require_user(&headers, &state).await?;
    if user.role != "root" {
        return Err(ApiError::unauthorized("root required"));
    }
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
                Some(uid),
                Some(&device_id),
                "device_revoked",
                None,
                None,
                None,
            )
            .await;
    }

    Ok(Json(serde_json::json!({ "status": "ok" })))
}

#[derive(Debug, serde::Deserialize)]
pub struct AuditQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct AuditRecord {
    pub id: i64,
    pub user_id: Option<String>,
    pub device_id: Option<String>,
    pub event: String,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub detail: Option<String>,
    pub ts: i64,
}

async fn list_audits(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuditQuery>,
) -> Result<Json<Vec<AuditRecord>>, ApiError> {
    let user = require_user(&headers, &state).await?;
    if user.role != "root" {
        return Err(ApiError::unauthorized("root required"));
    }
    let limit = query.limit.unwrap_or(100).clamp(1, 1000);
    let rows = sqlx::query(
        r#"
        SELECT id, user_id, device_id, event, ip, user_agent, detail, ts
        FROM login_audit
        ORDER BY ts DESC
        LIMIT ?1
        "#,
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(AuditRecord {
            id: row.get("id"),
            user_id: row.get("user_id"),
            device_id: row.get("device_id"),
            event: row.get("event"),
            ip: row.get("ip"),
            user_agent: row.get("user_agent"),
            detail: row.get("detail"),
            ts: row.get("ts"),
        });
    }
    Ok(Json(items))
}

async fn join_start(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<JoinStartResponse>, ApiError> {
    let user = require_user(&headers, &state).await?;
    if user.role != "root" {
        return Err(ApiError::unauthorized("root required"));
    }

    let token = Uuid::new_v4().to_string();
    let pin = generate_pin();
    let pin_hash = hash_pin(&pin)?;
    let now = Utc::now().timestamp();
    let expires_at = now + 600;

    sqlx::query(
        r#"
        INSERT INTO join_challenges (token, pin_hash, expires_at, created_at)
        VALUES (?1, ?2, ?3, ?4)
        "#,
    )
    .bind(&token)
    .bind(pin_hash)
    .bind(expires_at)
    .bind(now)
    .execute(&state.db)
    .await?;

    Ok(Json(JoinStartResponse {
        token,
        pin,
        expires_at,
    }))
}

fn generate_pin() -> String {
    let mut bytes = [0u8; 4];
    rand::thread_rng().fill_bytes(&mut bytes);
    let num = u32::from_le_bytes(bytes) % 1_000_000;
    format!("{:06}", num)
}

fn hash_pin(pin: &str) -> anyhow::Result<String> {
    let salt = argon2::password_hash::SaltString::generate(&mut rand::thread_rng());
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(pin.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?
        .to_string();
    Ok(hash)
}
