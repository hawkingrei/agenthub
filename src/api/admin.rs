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
                Some(&device_id),
                "device_revoked",
                Some(&format!("device_id={}", device_id)),
                extract_ip(&headers).as_deref(),
                extract_ua(&headers).as_deref(),
            )
            .await;
    }

    Ok(ok_response())
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
    let _user = require_root(&headers, &state).await?;
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
    let user = require_root(&headers, &state).await?;
    let passkey_enabled = payload.enabled;
    state.auth.set_passkey_enabled(passkey_enabled).await?;
    let detail = format!("enabled={passkey_enabled}");
    let _ = state
        .auth
        .record_audit(
            Some(&user.id),
            None,
            "passkey_config_updated",
            Some(&detail),
            extract_ip(&headers).as_deref(),
            extract_ua(&headers).as_deref(),
        )
        .await;
    Ok(ok_response())
}

async fn join_start(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<JoinStartResponse>, ApiError> {
    let user = require_root(&headers, &state).await?;
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
    .bind(&pin_hash)
    .bind(expires_at)
    .bind(now)
    .execute(&state.db)
    .await?;

    let detail = format!("expires_at={expires_at}");
    let _ = state
        .auth
        .record_audit(
            Some(&user.id),
            None,
            "join_challenge_created",
            Some(&detail),
            extract_ip(&headers).as_deref(),
            extract_ua(&headers).as_deref(),
        )
        .await;

    Ok(Json(JoinStartResponse {
        token,
        pin,
        expires_at,
    }))
}

fn generate_pin() -> String {
    let mut bytes = [0u8; 4];
    rand::rng().fill_bytes(&mut bytes);
    let num = u32::from_le_bytes(bytes) % 1_000_000;
    format!("{num:06}")
}

fn hash_pin(pin: &str) -> anyhow::Result<String> {
    let salt =
        argon2::password_hash::SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(pin.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?
        .to_string();
    Ok(hash)
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode, header};
    use sqlx::Row;
    use tower::util::ServiceExt;

    use crate::api::teams::tests::{build_test_state, create_auth_token};

    fn build_request(token: Option<&str>) -> Request<Body> {
        let mut builder = Request::builder().method(Method::POST).uri("/join/start");
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        builder.body(Body::empty()).expect("build request")
    }

    #[tokio::test]
    async fn join_start_records_audit_entry() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = super::router(state.clone());

        let response = app
            .oneshot(build_request(Some(&token)))
            .await
            .expect("execute request");
        assert_eq!(response.status(), StatusCode::OK);

        let challenge_count: i64 = sqlx::query("SELECT COUNT(*) AS count FROM join_challenges")
            .fetch_one(&state.db)
            .await
            .expect("count join challenges")
            .get("count");
        assert_eq!(challenge_count, 1);

        let row = sqlx::query(
            r#"
            SELECT event, detail
            FROM login_audit
            WHERE event = 'join_challenge_created'
            ORDER BY ts DESC
            LIMIT 1
            "#,
        )
        .fetch_one(&state.db)
        .await
        .expect("fetch join audit");
        let event: String = row.get("event");
        let detail: Option<String> = row.get("detail");
        assert_eq!(event, "join_challenge_created");
        assert!(
            detail
                .as_deref()
                .is_some_and(|value| value.starts_with("expires_at="))
        );
    }
}
