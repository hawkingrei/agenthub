use argon2::{Argon2, password_hash::PasswordHasher};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{delete, get, post, put},
};
use chrono::Utc;
use rand::Rng;
use sqlx::Row;
use uuid::Uuid;

use agenthub_auth_domain::UserCapability;

use crate::api::authz::{require_capability, require_root};
use crate::api::error::{ApiError, map_linker_error};
use crate::api::{extract_ip, extract_ua, ok_response};
use crate::linkers::{self, AppLinkerRecord, AppLinkerService, SlockConfigInput};
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

#[derive(Debug, Default, serde::Deserialize)]
pub struct MigrateTeamMessagesArchiveRequest {
    pub batch_size: Option<usize>,
}

#[derive(Debug, serde::Serialize)]
pub struct MigrateTeamMessagesArchiveResponse {
    pub team_conversation_messages: usize,
    pub team_run_events: usize,
    pub team_actor_messages: usize,
    pub agent_events: usize,
    pub aggregated_acp_messages: usize,
    pub total_documents: usize,
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

#[derive(Debug, serde::Deserialize)]
pub struct UpsertSlockLinkerRequest {
    pub api_origin: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub return_url: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct SlockLinkAttemptResponse {
    pub linker_id: String,
    pub state: String,
    pub expires_at: i64,
    pub return_url: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct ExchangeSlockCodeRequest {
    pub code: Option<String>,
    pub callback_url: Option<String>,
    pub state: Option<String>,
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
        .route("/linkers", get(list_linkers))
        .route("/linkers/slock", put(upsert_slock_linker))
        .route(
            "/linkers/slock/link_attempts",
            post(create_slock_link_attempt),
        )
        .route("/linkers/slock/exchange", post(exchange_slock_code))
        .route("/linkers/slock/userinfo", get(get_slock_userinfo))
        .route("/linkers/slock/channels", get(list_slock_channels))
        .route(
            "/linkers/slock/channels/{channel_id}/messages",
            get(list_slock_channel_messages),
        )
        .route(
            "/message_archive/team_messages/migrate",
            post(migrate_team_messages_archive),
        )
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

async fn list_linkers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AppLinkerRecord>>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::LinkersManage).await?;
    AppLinkerService::new(state.db.clone(), state.linker_http.clone())
        .list_linkers()
        .await
        .map(Json)
        .map_err(map_linker_error)
}

async fn upsert_slock_linker(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpsertSlockLinkerRequest>,
) -> Result<Json<AppLinkerRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::LinkersManage).await?;
    let record = AppLinkerService::new(state.db.clone(), state.linker_http.clone())
        .upsert_slock_config(
            &user.id,
            SlockConfigInput {
                api_origin: payload.api_origin,
                client_id: payload.client_id,
                client_secret: payload.client_secret,
                return_url: payload.return_url,
                scopes: payload.scopes,
            },
        )
        .await
        .map_err(map_linker_error)?;
    let detail = format!("linker_id={}", record.linker_id);
    let _ = state
        .auth
        .record_audit(
            Some(&user.id),
            None,
            "slock_linker_config_updated",
            Some(&detail),
            extract_ip(&headers).as_deref(),
            extract_ua(&headers).as_deref(),
        )
        .await;
    Ok(Json(record))
}

async fn create_slock_link_attempt(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SlockLinkAttemptResponse>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::LinkersManage).await?;
    let attempt = AppLinkerService::new(state.db.clone(), state.linker_http.clone())
        .create_slock_link_attempt(&user.id)
        .await
        .map_err(map_linker_error)?;
    let detail = format!(
        "linker_id={},expires_at={}",
        attempt.linker_id, attempt.expires_at
    );
    let _ = state
        .auth
        .record_audit(
            Some(&user.id),
            None,
            "slock_link_attempt_created",
            Some(&detail),
            extract_ip(&headers).as_deref(),
            extract_ua(&headers).as_deref(),
        )
        .await;
    Ok(Json(SlockLinkAttemptResponse {
        linker_id: attempt.linker_id,
        state: attempt.state,
        expires_at: attempt.expires_at,
        return_url: attempt.return_url,
    }))
}

async fn exchange_slock_code(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ExchangeSlockCodeRequest>,
) -> Result<Json<AppLinkerRecord>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::LinkersManage).await?;
    let code = linkers::parse_slock_exchange_code(
        payload.code.as_deref(),
        payload.callback_url.as_deref(),
        payload.state.as_deref(),
    )
    .map_err(map_linker_error)?;
    let record = AppLinkerService::new(state.db.clone(), state.linker_http.clone())
        .exchange_slock_code(Some(&user.id), code)
        .await
        .map_err(map_linker_error)?;
    let detail = format!(
        "linker_id={},principal_type={}",
        record.linker_id,
        record
            .principal
            .as_ref()
            .map(|principal| principal.principal_type.as_str())
            .unwrap_or("<none>")
    );
    let _ = state
        .auth
        .record_audit(
            Some(&user.id),
            None,
            "slock_linker_exchange_completed",
            Some(&detail),
            extract_ip(&headers).as_deref(),
            extract_ua(&headers).as_deref(),
        )
        .await;
    Ok(Json(record))
}

async fn get_slock_userinfo(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AppLinkerRecord>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::LinkersManage).await?;
    let record = AppLinkerService::new(state.db.clone(), state.linker_http.clone())
        .get_slock_linker()
        .await
        .map_err(map_linker_error)?
        .ok_or_else(|| ApiError::not_found("Slock linker is not configured"))?;
    Ok(Json(record))
}

async fn list_slock_channels(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::LinkersManage).await?;
    AppLinkerService::new(state.db.clone(), state.linker_http.clone())
        .list_slock_channels()
        .await
        .map(Json)
        .map_err(map_linker_error)
}

async fn list_slock_channel_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(channel_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::LinkersManage).await?;
    AppLinkerService::new(state.db.clone(), state.linker_http.clone())
        .list_slock_channel_messages(&channel_id)
        .await
        .map(Json)
        .map_err(map_linker_error)
}

async fn migrate_team_messages_archive(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: Option<Json<MigrateTeamMessagesArchiveRequest>>,
) -> Result<Json<MigrateTeamMessagesArchiveResponse>, ApiError> {
    let user = require_root(&headers, &state).await?;
    let payload = payload.map(|Json(payload)| payload).unwrap_or_default();
    let report = match state
        .teams
        .migrate_team_messages_to_archive(payload.batch_size.unwrap_or(500))
        .await
    {
        Ok(report) => report,
        Err(err) if err.to_string() == "message archive is not configured" => {
            return Err(ApiError::conflict(
                "message archive is not configured; enable a supported message archive backend before migrating team messages",
            ));
        }
        Err(err) => return Err(err.into()),
    };
    let total_documents = report.total_documents();
    let detail = format!(
        "team_conversation_messages={},team_run_events={},team_actor_messages={},agent_events={},aggregated_acp_messages={},total_documents={}",
        report.team_conversation_messages,
        report.team_run_events,
        report.team_actor_messages,
        report.agent_events,
        report.aggregated_acp_messages,
        total_documents
    );
    let _ = state
        .auth
        .record_audit(
            Some(&user.id),
            None,
            "message_archive_team_messages_migrated",
            Some(&detail),
            extract_ip(&headers).as_deref(),
            extract_ua(&headers).as_deref(),
        )
        .await;
    Ok(Json(MigrateTeamMessagesArchiveResponse {
        team_conversation_messages: report.team_conversation_messages,
        team_run_events: report.team_run_events,
        team_actor_messages: report.team_actor_messages,
        agent_events: report.agent_events,
        aggregated_acp_messages: report.aggregated_acp_messages,
        total_documents,
    }))
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
    use std::sync::Arc;

    use agenthub_message_archive::{
        MessageArchiveStore, MessageDocument, MessageSearchHit, MessageSearchQuery,
    };
    use async_trait::async_trait;
    use axum::body::Body;
    use axum::body::to_bytes;
    use axum::http::{Method, Request, StatusCode, header};
    use chrono::Utc;
    use serde_json::{Value, json};
    use sqlx::Row;
    use tower::util::ServiceExt;
    use uuid::Uuid;

    use crate::api::teams::tests::{
        build_test_state, build_test_state_with_message_archive, create_auth_token,
    };
    use crate::team::TeamDefinitionConfig;

    #[derive(Default)]
    struct NoopMessageArchive;

    #[async_trait]
    impl MessageArchiveStore for NoopMessageArchive {
        async fn ensure_ready(&self) -> anyhow::Result<()> {
            Ok(())
        }

        async fn append_documents(&self, _documents: &[MessageDocument]) -> anyhow::Result<()> {
            Ok(())
        }

        async fn contains_document(&self, _document_id: &str) -> anyhow::Result<bool> {
            Ok(false)
        }

        async fn search(
            &self,
            _query: &MessageSearchQuery,
        ) -> anyhow::Result<Vec<MessageSearchHit>> {
            Ok(Vec::new())
        }
    }

    fn build_request(token: Option<&str>) -> Request<Body> {
        let mut builder = Request::builder().method(Method::POST).uri("/join/start");
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        builder.body(Body::empty()).expect("build request")
    }

    fn build_json_request(path: &str, token: Option<&str>, payload: Value) -> Request<Body> {
        build_method_json_request(Method::POST, path, token, payload)
    }

    fn build_method_json_request(
        method: Method,
        path: &str,
        token: Option<&str>,
        payload: Value,
    ) -> Request<Body> {
        let mut builder = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(payload.to_string()))
            .expect("build json request")
    }

    fn build_empty_request(method: Method, path: &str, token: Option<&str>) -> Request<Body> {
        let mut builder = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        builder.body(Body::empty()).expect("build empty request")
    }

    fn build_empty_post_request(path: &str, token: Option<&str>) -> Request<Body> {
        build_empty_request(Method::POST, path, token)
    }

    async fn decode_json_body(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        serde_json::from_slice(&bytes).expect("decode response body")
    }

    async fn create_auth_token_with_role(state: &crate::state::AppState, role: &str) -> String {
        let user_id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO users (id, username, display_name, role, password_hash, created_at)
            VALUES (?1, ?2, ?3, ?4, NULL, ?5)
            "#,
        )
        .bind(&user_id)
        .bind(format!("{role}-{}", Uuid::new_v4()))
        .bind("Admin Route Test User")
        .bind(role)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert user");

        state
            .auth
            .create_session(&user_id)
            .await
            .expect("create token")
    }

    fn slock_config_payload() -> Value {
        json!({
            "api_origin": "https://api.slock.test/",
            "client_id": "agenthub-dev",
            "client_secret": "slock-secret",
            "return_url": "https://agenthub.example/api/linkers/slock/callback",
            "scopes": ["identity", "openid", "profile"]
        })
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

    #[tokio::test]
    async fn migrate_team_messages_archive_requires_root_and_records_audit() {
        let state = build_test_state_with_message_archive(Arc::new(NoopMessageArchive)).await;
        let team = state
            .teams
            .create_team(TeamDefinitionConfig {
                name: "admin-archive-migration-team".to_string(),
                description: None,
                spec: json!({"entrypoint":"coordinator_plan","members":[{"member_id":"coordinator"}]}),
            })
            .await
            .expect("create team");
        let (task, _) = state
            .teams
            .create_task(
                &team.id,
                "Admin archive migration",
                "user",
                json!({}),
                "group_chat",
                None,
            )
            .await
            .expect("create task");
        state
            .teams
            .append_task_conversation_message(
                &task.id,
                "user",
                Some("coordinator"),
                "to_coordinator",
                json!({"type":"chat_message","text":"archive via admin route"}),
            )
            .await
            .expect("append conversation message");

        let app = super::router(state.clone());
        let unauthorized = app
            .clone()
            .oneshot(build_json_request(
                "/message_archive/team_messages/migrate",
                None,
                json!({}),
            ))
            .await
            .expect("execute unauthorized request");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let token = create_auth_token(&state).await;
        let response = app
            .oneshot(build_empty_post_request(
                "/message_archive/team_messages/migrate",
                Some(&token),
            ))
            .await
            .expect("execute migration request");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        let body: Value = serde_json::from_slice(&bytes).expect("decode response body");
        assert_eq!(body["team_conversation_messages"], 1);
        assert_eq!(body["team_run_events"], 0);
        assert_eq!(body["team_actor_messages"], 0);
        assert_eq!(body["agent_events"], 0);
        assert_eq!(body["aggregated_acp_messages"], 0);
        assert_eq!(body["total_documents"], 1);

        let row = sqlx::query(
            r#"
            SELECT event, detail
            FROM login_audit
            WHERE event = 'message_archive_team_messages_migrated'
            ORDER BY ts DESC
            LIMIT 1
            "#,
        )
        .fetch_one(&state.db)
        .await
        .expect("fetch migration audit");
        let event: String = row.get("event");
        let detail: Option<String> = row.get("detail");
        assert_eq!(event, "message_archive_team_messages_migrated");
        assert!(
            detail
                .as_deref()
                .is_some_and(|value| value.contains("total_documents=1"))
        );
        assert!(detail.as_deref().is_some_and(|value| {
            value.contains("agent_events=0") && value.contains("aggregated_acp_messages=0")
        }));
    }

    #[tokio::test]
    async fn slock_linker_routes_require_linkers_manage_and_do_not_expose_secrets() {
        let state = build_test_state().await;
        let admin_token = create_auth_token_with_role(&state, "admin").await;
        let operator_token = create_auth_token_with_role(&state, "operator").await;
        let app = super::router(state.clone());

        let unauthorized = app
            .clone()
            .oneshot(build_empty_request(
                Method::GET,
                "/linkers",
                Some(&operator_token),
            ))
            .await
            .expect("execute operator list linkers request");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
        let unauthorized_body = decode_json_body(unauthorized).await;
        assert_eq!(unauthorized_body["error"], json!("linkers:manage required"));

        let response = app
            .clone()
            .oneshot(build_method_json_request(
                Method::PUT,
                "/linkers/slock",
                Some(&admin_token),
                slock_config_payload(),
            ))
            .await
            .expect("execute upsert slock linker request");
        assert_eq!(response.status(), StatusCode::OK);
        let body = decode_json_body(response).await;
        assert_eq!(body["linker_id"], json!("slock-primary"));
        assert_eq!(body["connector_id"], json!("slock"));
        assert_eq!(body["status"], json!("configured"));
        assert_eq!(body["api_origin"], json!("https://api.slock.test"));
        assert_eq!(body["client_id"], json!("agenthub-dev"));
        assert_eq!(body["client_secret_configured"], json!(true));
        assert_eq!(body["token_configured"], json!(false));
        assert!(body.get("client_secret").is_none());
        assert!(body.get("access_token").is_none());

        let response = app
            .oneshot(build_empty_request(
                Method::GET,
                "/linkers",
                Some(&admin_token),
            ))
            .await
            .expect("execute list linkers request");
        assert_eq!(response.status(), StatusCode::OK);
        let body = decode_json_body(response).await;
        let items = body.as_array().expect("linkers response array");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["linker_id"], json!("slock-primary"));
        assert_eq!(items[0]["client_secret_configured"], json!(true));
        assert!(items[0].get("client_secret").is_none());
        assert!(items[0].get("access_token").is_none());
    }

    #[tokio::test]
    async fn slock_link_attempt_requires_config_and_persists_state() {
        let state = build_test_state().await;
        let root_token = create_auth_token(&state).await;
        let app = super::router(state.clone());

        let missing_config = app
            .clone()
            .oneshot(build_empty_post_request(
                "/linkers/slock/link_attempts",
                Some(&root_token),
            ))
            .await
            .expect("execute missing config link attempt request");
        assert_eq!(missing_config.status(), StatusCode::CONFLICT);
        let missing_config_body = decode_json_body(missing_config).await;
        assert_eq!(
            missing_config_body["error"],
            json!("Slock linker is not configured")
        );

        let configured = app
            .clone()
            .oneshot(build_method_json_request(
                Method::PUT,
                "/linkers/slock",
                Some(&root_token),
                slock_config_payload(),
            ))
            .await
            .expect("execute upsert slock linker request");
        assert_eq!(configured.status(), StatusCode::OK);

        let response = app
            .oneshot(build_empty_post_request(
                "/linkers/slock/link_attempts",
                Some(&root_token),
            ))
            .await
            .expect("execute link attempt request");
        assert_eq!(response.status(), StatusCode::OK);
        let body = decode_json_body(response).await;
        let state_value = body["state"].as_str().expect("state string");
        assert_eq!(body["linker_id"], json!("slock-primary"));
        assert_eq!(
            body["return_url"],
            json!("https://agenthub.example/api/linkers/slock/callback")
        );
        assert!(Uuid::parse_str(state_value).is_ok());

        let row = sqlx::query(
            r#"
            SELECT linker_id, expires_at
            FROM app_linker_attempts
            WHERE state = ?1
            "#,
        )
        .bind(state_value)
        .fetch_one(&state.db)
        .await
        .expect("fetch link attempt");
        let linker_id: String = row.get("linker_id");
        let expires_at: i64 = row.get("expires_at");
        assert_eq!(linker_id, "slock-primary");
        assert!(expires_at >= Utc::now().timestamp());
    }

    #[tokio::test]
    async fn slock_channel_routes_report_missing_resource_contract() {
        let state = build_test_state().await;
        let root_token = create_auth_token(&state).await;
        let app = super::router(state.clone());

        let configured = app
            .clone()
            .oneshot(build_method_json_request(
                Method::PUT,
                "/linkers/slock",
                Some(&root_token),
                slock_config_payload(),
            ))
            .await
            .expect("execute upsert slock linker request");
        assert_eq!(configured.status(), StatusCode::OK);

        sqlx::query(
            r#"
            UPDATE app_linkers
            SET status = 'connected'
            WHERE id = 'slock-primary'
            "#,
        )
        .execute(&state.db)
        .await
        .expect("mark slock linker connected");
        sqlx::query(
            r#"
            UPDATE app_linker_secrets
            SET access_token = 'slock_at_test',
                token_type = 'Bearer',
                scope = 'identity openid profile',
                expires_at = ?1
            WHERE linker_id = 'slock-primary'
            "#,
        )
        .bind(Utc::now().timestamp() + 3600)
        .execute(&state.db)
        .await
        .expect("store test slock token");

        let response = app
            .oneshot(build_empty_request(
                Method::GET,
                "/linkers/slock/channels",
                Some(&root_token),
            ))
            .await
            .expect("execute slock channels request");
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = decode_json_body(response).await;
        assert_eq!(
            body["error"],
            json!(
                "Slock channel resource API is not configured yet; complete the Slock resource endpoint contract before enabling channel reads"
            )
        );
    }

    #[tokio::test]
    async fn migrate_team_messages_archive_reports_missing_archive_as_conflict() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let response = super::router(state)
            .oneshot(build_json_request(
                "/message_archive/team_messages/migrate",
                Some(&token),
                json!({}),
            ))
            .await
            .expect("execute migration request");

        assert_eq!(response.status(), StatusCode::CONFLICT);
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        let body: Value = serde_json::from_slice(&bytes).expect("decode response body");
        assert_eq!(
            body["error"].as_str(),
            Some(
                "message archive is not configured; enable a supported message archive backend before migrating team messages"
            )
        );
    }
}
