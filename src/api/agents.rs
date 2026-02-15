use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{delete, get, post},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::acp::AcpActorSkillContext;
use crate::acp::AcpPermissionRecord;
use crate::actor_runtime::{
    DEFAULT_ACTOR_CHANNEL, default_actor_cli_path, normalize_actor_cli_path,
};
use crate::agent::{AgentConfig, AgentRecord, WorktreeMode};
use crate::api::authz::require_user;
use crate::api::error::ApiError;
use crate::state::AppState;

const ACTOR_CONTEXT_RUNNING_CONFLICT: &str = "cannot start with new actor context";

#[derive(Debug, serde::Deserialize)]
pub struct CreateAgentRequest {
    pub name: String,
    pub workdir: String,
    pub command: String,
    pub args: Vec<String>,
    pub worktree_mode: Option<String>,
    pub worktree_repo: Option<String>,
    pub worktree_ref: Option<String>,
    pub code_mode: Option<bool>,
}

#[derive(Debug, serde::Serialize)]
pub struct StartAgentResponse {
    pub session_id: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct StartAgentRequest {
    pub actor_runtime: Option<StartAgentActorRuntimeRequest>,
}

#[derive(Debug, serde::Deserialize)]
pub struct StartAgentActorRuntimeRequest {
    pub run_id: String,
    pub actor_id: String,
    pub channel: Option<String>,
    pub actor_cli_path: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ListEventsQuery {
    pub limit: Option<i64>,
    pub session_id: Option<String>,
    pub before_id: Option<i64>,
}

#[derive(Debug, serde::Deserialize)]
pub struct SetCodeModeRequest {
    pub code_mode: bool,
}

#[derive(Debug, serde::Deserialize)]
pub struct SendInputRequest {
    pub input: String,
    pub message_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ClearSessionRequest {
    pub provider: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct SetAcpModeRequest {
    pub mode_id: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct SetAcpModelRequest {
    pub model_id: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct SetAcpConfigRequest {
    pub config_id: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct PermissionListQuery {
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PermissionResponseRequest {
    pub option_id: Option<String>,
    pub outcome: Option<String>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", post(create_agent).get(list_agents))
        .route("/:id", get(get_agent))
        .route("/:id/start", post(start_agent))
        .route("/:id/stop", post(stop_agent))
        .route("/:id/input", post(send_input))
        .route("/:id", delete(delete_agent))
        .route("/:id/events", get(list_events))
        .route("/:id/code_mode", post(set_code_mode))
        .route("/:id/acp/session/clear", post(clear_acp_session))
        .route("/:id/acp/mode", post(set_acp_mode))
        .route("/:id/acp/model", post(set_acp_model))
        .route("/:id/acp/config", post(set_acp_config))
        .route("/:id/acp/cancel", post(cancel_acp))
        .route("/:id/permissions", get(list_permissions))
        .route(
            "/:id/permissions/:permission_id/respond",
            post(respond_permission),
        )
        .with_state(state)
}

async fn create_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateAgentRequest>,
) -> Result<Json<AgentRecord>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let worktree_mode = parse_worktree_mode(payload.worktree_mode.as_deref());
    let workdir = resolve_create_agent_workdir(
        &payload.workdir,
        &payload.name,
        &worktree_mode,
        &state.default_worktree_root,
    )?;
    let config = AgentConfig {
        name: payload.name,
        workdir,
        command: payload.command,
        args: payload.args,
        worktree_mode,
        worktree_repo: payload.worktree_repo,
        worktree_ref: payload.worktree_ref,
        code_mode: payload.code_mode.unwrap_or(true),
    };
    let agent = state.agents.create_agent(config).await?;
    Ok(Json(agent))
}

async fn list_agents(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AgentRecord>>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let agents = state.agents.list_agents().await?;
    Ok(Json(agents))
}

async fn get_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
) -> Result<Json<AgentRecord>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let agent = state.agents.get_agent(&agent_id).await?;
    Ok(Json(agent))
}

async fn start_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    payload: Option<Json<StartAgentRequest>>,
) -> Result<Json<StartAgentResponse>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let actor_context = parse_start_actor_runtime_context(payload.map(|Json(body)| body))?;
    let session_id = if let Some(actor_context) = actor_context {
        match state
            .agents
            .start_agent_with_actor_context(&agent_id, Some(actor_context))
            .await
        {
            Ok(session_id) => session_id,
            Err(err) => {
                let message = err.to_string();
                if message.contains(ACTOR_CONTEXT_RUNNING_CONFLICT) {
                    return Err(ApiError::conflict(&message));
                }
                return Err(err.into());
            }
        }
    } else {
        state.agents.start_agent(&agent_id).await?
    };
    Ok(Json(StartAgentResponse { session_id }))
}

async fn stop_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    state.agents.stop_agent(&agent_id).await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn delete_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let _ = state.agents.stop_agent(&agent_id).await;
    state.agents.delete_agent(&agent_id).await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn send_input(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<SendInputRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    state
        .agents
        .send_input(&agent_id, &payload.input, payload.message_id.as_deref())
        .await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn list_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Query(query): Query<ListEventsQuery>,
) -> Result<Json<Vec<crate::agent::AgentEvent>>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let limit = query.limit.unwrap_or(500).clamp(1, 1000);
    let before_id = query.before_id;
    let events = if let Some(session_id) = query.session_id.as_deref() {
        state
            .agents
            .list_events_for_session(&agent_id, session_id, limit, before_id)
            .await?
    } else {
        state
            .agents
            .list_events(&agent_id, limit, before_id)
            .await?
    };
    Ok(Json(events))
}

async fn set_code_mode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<SetCodeModeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    state
        .agents
        .set_code_mode(&agent_id, payload.code_mode)
        .await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn clear_acp_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<ClearSessionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let provider = match payload.provider {
        Some(provider) => provider,
        None => match state.agents.get_agent(&agent_id).await {
            Ok(agent) => state
                .agents
                .acp_provider_for_agent(&agent.command, &agent.args)
                .unwrap_or("codex")
                .to_string(),
            Err(err) => {
                if err
                    .downcast_ref::<sqlx::Error>()
                    .map(|e| matches!(e, sqlx::Error::RowNotFound))
                    .unwrap_or(false)
                {
                    "codex".to_string()
                } else {
                    return Err(err.into());
                }
            }
        },
    };
    state
        .agents
        .clear_persistent_session(&agent_id, &provider)
        .await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn set_acp_mode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<SetAcpModeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    state
        .agents
        .set_acp_mode(&agent_id, &payload.mode_id)
        .await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn set_acp_model(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<SetAcpModelRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    state
        .agents
        .set_acp_model(&agent_id, &payload.model_id)
        .await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn set_acp_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<SetAcpConfigRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    state
        .agents
        .set_acp_config(&agent_id, &payload.config_id, &payload.value)
        .await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn cancel_acp(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    state.agents.cancel_acp(&agent_id).await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn list_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Query(query): Query<PermissionListQuery>,
) -> Result<Json<Vec<AcpPermissionRecord>>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let status = query.status.as_deref();
    let records = state.acp_permissions.list(&agent_id, status).await?;
    Ok(Json(records))
}

async fn respond_permission(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((agent_id, permission_id)): Path<(String, String)>,
    Json(payload): Json<PermissionResponseRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let _ = agent_id;
    let outcome = if let Some(option_id) = payload.option_id.clone() {
        agent_client_protocol::RequestPermissionOutcome::Selected(
            agent_client_protocol::SelectedPermissionOutcome::new(option_id.clone()),
        )
    } else if payload.outcome.as_deref() == Some("cancelled") {
        agent_client_protocol::RequestPermissionOutcome::Cancelled
    } else {
        agent_client_protocol::RequestPermissionOutcome::Cancelled
    };
    state
        .acp_permissions
        .respond(&permission_id, outcome, payload.option_id)
        .await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

fn parse_worktree_mode(value: Option<&str>) -> WorktreeMode {
    match value {
        Some("create_worktree") => WorktreeMode::CreateWorktree,
        Some("reuse_worktree") => WorktreeMode::ReuseWorktree,
        _ => WorktreeMode::UseExisting,
    }
}

fn resolve_create_agent_workdir(
    requested_workdir: &str,
    agent_name: &str,
    worktree_mode: &WorktreeMode,
    default_worktree_root: &str,
) -> Result<String, ApiError> {
    let trimmed = requested_workdir.trim();
    if !trimmed.is_empty() {
        return Ok(trimmed.to_string());
    }
    if !matches!(worktree_mode, WorktreeMode::CreateWorktree) {
        return Err(ApiError::bad_request("workdir is required"));
    }
    Ok(default_worktree_path(agent_name, default_worktree_root))
}

fn default_worktree_path(agent_name: &str, default_worktree_root: &str) -> String {
    let root = default_worktree_root
        .trim()
        .trim_end_matches('/')
        .trim_end_matches('\\');
    let name = sanitize_worktree_segment(agent_name);
    let suffix = Uuid::new_v4().simple().to_string();
    let segment = format!("{name}-{}", &suffix[..8]);
    std::path::PathBuf::from(root)
        .join(segment)
        .to_string_lossy()
        .to_string()
}

fn sanitize_worktree_segment(value: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
            continue;
        }
        if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "agent".to_string()
    } else {
        trimmed.to_string()
    }
}

fn parse_start_actor_runtime_context(
    payload: Option<StartAgentRequest>,
) -> Result<Option<AcpActorSkillContext>, ApiError> {
    let Some(payload) = payload else {
        return Ok(None);
    };
    let Some(actor_runtime) = payload.actor_runtime else {
        return Ok(None);
    };

    let run_id = actor_runtime.run_id.trim();
    if run_id.is_empty() {
        return Err(ApiError::bad_request("actor_runtime.run_id is required"));
    }
    let actor_id = actor_runtime.actor_id.trim();
    if actor_id.is_empty() {
        return Err(ApiError::bad_request("actor_runtime.actor_id is required"));
    }
    let default_channel = actor_runtime
        .channel
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_ACTOR_CHANNEL)
        .to_string();
    let actor_cli_path = match actor_runtime.actor_cli_path.as_deref() {
        Some(path) => normalize_actor_cli_path(Some(path))
            .map_err(|err| ApiError::bad_request(err.to_string().as_str()))?,
        None => default_actor_cli_path()?,
    };

    Ok(Some(AcpActorSkillContext {
        run_id: run_id.to_string(),
        actor_id: actor_id.to_string(),
        default_channel,
        actor_cli_path,
    }))
}

#[cfg(test)]
mod tests {
    use std::process::Command as StdCommand;
    use std::sync::Arc;
    use std::time::Duration;

    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request, StatusCode, header};
    use serde_json::{Value, json};
    use sqlx::SqlitePool;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use tower::ServiceExt;
    use uuid::Uuid;

    use crate::acp::AcpPermissionService;
    use crate::actor_runtime::default_actor_cli_path;
    use crate::agent::AgentManager;
    use crate::auth::AuthService;
    use crate::config::{AppConfig, PushConfig, WebConfig};
    use crate::push::PushService;
    use crate::state::AppState;
    use crate::team::TeamManager;

    use super::{
        StartAgentActorRuntimeRequest, StartAgentRequest, WorktreeMode,
        parse_start_actor_runtime_context, parse_worktree_mode, resolve_create_agent_workdir,
        router, sanitize_worktree_segment,
    };

    #[test]
    fn parse_worktree_mode_defaults() {
        assert!(matches!(
            parse_worktree_mode(None),
            WorktreeMode::UseExisting
        ));
        assert!(matches!(
            parse_worktree_mode(Some("unknown")),
            WorktreeMode::UseExisting
        ));
    }

    #[test]
    fn parse_worktree_mode_explicit() {
        assert!(matches!(
            parse_worktree_mode(Some("create_worktree")),
            WorktreeMode::CreateWorktree
        ));
        assert!(matches!(
            parse_worktree_mode(Some("reuse_worktree")),
            WorktreeMode::ReuseWorktree
        ));
    }

    #[test]
    fn resolve_create_agent_workdir_uses_explicit_value() {
        let resolved = resolve_create_agent_workdir(
            " /tmp/work ",
            "planner",
            &WorktreeMode::CreateWorktree,
            "~/.agenthub/worktrees",
        )
        .expect("resolve workdir");
        assert_eq!(resolved, "/tmp/work");
    }

    #[test]
    fn resolve_create_agent_workdir_generates_default_for_create_mode() {
        let resolved = resolve_create_agent_workdir(
            "",
            "Team Planner",
            &WorktreeMode::CreateWorktree,
            "~/.agenthub/worktrees",
        )
        .expect("resolve default workdir");
        assert!(resolved.starts_with("~/.agenthub/worktrees/team-planner-"));
    }

    #[test]
    fn resolve_create_agent_workdir_rejects_blank_for_non_create_mode() {
        let err = resolve_create_agent_workdir(
            "",
            "planner",
            &WorktreeMode::ReuseWorktree,
            "~/.agenthub/worktrees",
        )
        .expect_err("blank workdir should be rejected");
        let _ = err;
    }

    #[test]
    fn sanitize_worktree_segment_trims_mixed_edge_separators() {
        let sanitized = sanitize_worktree_segment("_-...Planner Team...-_");
        assert_eq!(sanitized, "planner-team");
    }

    #[test]
    fn sanitize_worktree_segment_collapses_repeated_internal_separators() {
        assert_eq!(sanitize_worktree_segment("agent...name"), "agent-name");
        assert_eq!(sanitize_worktree_segment("agent__name"), "agent-name");
        assert_eq!(sanitize_worktree_segment("agent._-name"), "agent-name");
        assert_eq!(
            sanitize_worktree_segment("agent   ---   name"),
            "agent-name"
        );
    }

    #[test]
    fn parse_start_actor_runtime_context_accepts_valid_payload() {
        let default_cli = default_actor_cli_path().expect("default actor cli path");
        let context = parse_start_actor_runtime_context(Some(StartAgentRequest {
            actor_runtime: Some(StartAgentActorRuntimeRequest {
                run_id: "run-7".to_string(),
                actor_id: "planner".to_string(),
                channel: Some("coordination".to_string()),
                actor_cli_path: Some(default_cli.clone()),
            }),
        }))
        .expect("parse actor runtime context")
        .expect("context");
        assert_eq!(context.run_id, "run-7");
        assert_eq!(context.actor_id, "planner");
        assert_eq!(context.default_channel, "coordination");
        assert_eq!(context.actor_cli_path, default_cli);
    }

    #[test]
    fn parse_start_actor_runtime_context_defaults_channel_and_cli_path() {
        let context = parse_start_actor_runtime_context(Some(StartAgentRequest {
            actor_runtime: Some(StartAgentActorRuntimeRequest {
                run_id: "run-9".to_string(),
                actor_id: "planner".to_string(),
                channel: None,
                actor_cli_path: None,
            }),
        }))
        .expect("parse actor runtime context")
        .expect("context");
        assert_eq!(context.default_channel, "default");
        assert_eq!(
            context.actor_cli_path,
            default_actor_cli_path().expect("default actor cli path")
        );
    }

    #[test]
    fn parse_start_actor_runtime_context_rejects_blank_required_fields() {
        let run_id_err = parse_start_actor_runtime_context(Some(StartAgentRequest {
            actor_runtime: Some(StartAgentActorRuntimeRequest {
                run_id: " ".to_string(),
                actor_id: "planner".to_string(),
                channel: None,
                actor_cli_path: None,
            }),
        }))
        .expect_err("run_id should be required");
        let _ = run_id_err;

        let actor_id_err = parse_start_actor_runtime_context(Some(StartAgentRequest {
            actor_runtime: Some(StartAgentActorRuntimeRequest {
                run_id: "run-2".to_string(),
                actor_id: " ".to_string(),
                channel: None,
                actor_cli_path: None,
            }),
        }))
        .expect_err("actor_id should be required");
        let _ = actor_id_err;
    }

    #[test]
    fn parse_start_actor_runtime_context_rejects_untrusted_actor_cli_path() {
        let err = parse_start_actor_runtime_context(Some(StartAgentRequest {
            actor_runtime: Some(StartAgentActorRuntimeRequest {
                run_id: "run-10".to_string(),
                actor_id: "planner".to_string(),
                channel: None,
                actor_cli_path: Some("/tmp/not-allowed-agenthub".to_string()),
            }),
        }))
        .expect_err("actor_cli_path should be validated");
        let _ = err;
    }

    async fn create_test_db() -> SqlitePool {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect sqlite")
    }

    async fn init_test_schema(db: &SqlitePool) {
        sqlx::query(
            r#"
            CREATE TABLE users (
                id TEXT PRIMARY KEY,
                username TEXT NOT NULL UNIQUE,
                display_name TEXT NOT NULL,
                role TEXT NOT NULL,
                password_hash TEXT,
                created_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(db)
        .await
        .expect("create users");

        sqlx::query(
            r#"
            CREATE TABLE auth_sessions (
                token TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                revoked_at INTEGER,
                FOREIGN KEY(user_id) REFERENCES users(id)
            )
            "#,
        )
        .execute(db)
        .await
        .expect("create auth_sessions");

        sqlx::query(
            r#"
            CREATE TABLE agents (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                workdir TEXT NOT NULL,
                command TEXT NOT NULL,
                args TEXT NOT NULL,
                worktree_mode TEXT NOT NULL,
                worktree_repo TEXT,
                worktree_ref TEXT,
                code_mode INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(db)
        .await
        .expect("create agents");

        sqlx::query(
            r#"
            CREATE TABLE safe_paths (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                created_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(db)
        .await
        .expect("create safe_paths");

        sqlx::query(
            r#"
            CREATE TABLE agent_sessions (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                status TEXT NOT NULL,
                started_at INTEGER NOT NULL,
                ended_at INTEGER,
                FOREIGN KEY(agent_id) REFERENCES agents(id)
            )
            "#,
        )
        .execute(db)
        .await
        .expect("create agent_sessions");

        sqlx::query(
            r#"
            CREATE TABLE agent_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                seq TEXT NOT NULL,
                ts INTEGER NOT NULL,
                stream TEXT NOT NULL,
                message TEXT NOT NULL,
                FOREIGN KEY(agent_id) REFERENCES agents(id),
                FOREIGN KEY(session_id) REFERENCES agent_sessions(id)
            )
            "#,
        )
        .execute(db)
        .await
        .expect("create agent_events");
    }

    async fn build_test_state() -> AppState {
        let db = create_test_db().await;
        init_test_schema(&db).await;
        let keys_dir = std::env::temp_dir().join(format!("agenthub-api-agents-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&keys_dir).expect("create keys dir");
        let keys_path = keys_dir.join("vapid.json");
        let config = AppConfig {
            web: Some(WebConfig {
                rp_id: Some("localhost".to_string()),
                rp_origin: Some("http://localhost:8080".to_string()),
                rp_name: Some("AgentHub Test".to_string()),
            }),
            push: Some(PushConfig {
                subject: Some("mailto:test@example.com".to_string()),
                keys_path: Some(keys_path.to_string_lossy().to_string()),
            }),
            ..Default::default()
        };
        let push = Arc::new(PushService::new(db.clone(), &config).expect("create push service"));
        let _ = std::fs::remove_dir_all(&keys_dir);
        let auth = Arc::new(
            AuthService::new(db.clone(), &config)
                .await
                .expect("create auth service"),
        );
        let permissions = Arc::new(AcpPermissionService::new(db.clone()));
        let agents = Arc::new(AgentManager::new(
            db.clone(),
            push.clone(),
            Vec::new(),
            "agenthub-codex-acp".to_string(),
            None,
            permissions.clone(),
            auth.clone(),
        ));
        let teams = Arc::new(TeamManager::new(db.clone()));
        AppState {
            db,
            agents,
            teams,
            push,
            auth,
            acp_permissions: permissions,
            default_worktree_root: config.default_worktree_root(),
        }
    }

    async fn create_auth_token(state: &AppState) -> String {
        let user_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO users (id, username, display_name, role, password_hash, created_at)
            VALUES (?1, ?2, ?3, 'root', NULL, ?4)
            "#,
        )
        .bind(&user_id)
        .bind(format!("root-{}", Uuid::new_v4()))
        .bind("Root")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert user");
        state
            .auth
            .create_session(&user_id)
            .await
            .expect("create session token")
    }

    fn build_json_request(
        method: Method,
        path: &str,
        token: Option<&str>,
        payload: Option<Value>,
    ) -> Request<Body> {
        let mut builder = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        match payload {
            Some(payload) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))
                .expect("build json request"),
            None => builder.body(Body::empty()).expect("build request"),
        }
    }

    async fn decode_json_body(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        serde_json::from_slice(&bytes).expect("decode response json")
    }

    fn run_git(repo_dir: &std::path::Path, args: &[&str]) {
        let status = StdCommand::new("git")
            .arg("-C")
            .arg(repo_dir)
            .args(args)
            .status()
            .expect("run git command");
        assert!(status.success(), "git command failed: {:?}", args);
    }

    #[tokio::test]
    async fn create_worktree_agent_can_start_again_after_stop() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());

        let base =
            std::env::temp_dir().join(format!("agenthub-create-worktree-{}", Uuid::new_v4()));
        let repo_dir = base.join("repo");
        let workdir = base.join("worktree-agent");
        std::fs::create_dir_all(&repo_dir).expect("create repo dir");

        run_git(&repo_dir, &["init"]);
        run_git(
            &repo_dir,
            &["config", "user.email", "agenthub-test@example.com"],
        );
        run_git(&repo_dir, &["config", "user.name", "AgentHub Test"]);
        std::fs::write(repo_dir.join("README.md"), "seed\n").expect("write seed file");
        run_git(&repo_dir, &["add", "README.md"]);
        run_git(&repo_dir, &["commit", "-m", "init"]);

        let now = chrono::Utc::now().timestamp();
        sqlx::query("INSERT INTO safe_paths (path, created_at) VALUES (?1, ?2)")
            .bind(repo_dir.to_string_lossy().to_string())
            .bind(now)
            .execute(&state.db)
            .await
            .expect("insert repo safe path");
        sqlx::query("INSERT INTO safe_paths (path, created_at) VALUES (?1, ?2)")
            .bind(workdir.to_string_lossy().to_string())
            .bind(now)
            .execute(&state.db)
            .await
            .expect("insert workdir safe path");

        let create_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "create-worktree-restart",
                    "workdir": workdir.to_string_lossy().to_string(),
                    "command": "/bin/sh",
                    "args": ["-lc", "sleep 30"],
                    "worktree_mode": "create_worktree",
                    "worktree_repo": repo_dir.to_string_lossy().to_string(),
                    "worktree_ref": "HEAD",
                    "code_mode": true
                })),
            ))
            .await
            .expect("create create_worktree agent");
        assert_eq!(create_resp.status(), StatusCode::OK);
        let created = decode_json_body(create_resp).await;
        let agent_id = created["id"].as_str().expect("agent id");

        let start_first = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/start"),
                Some(&token),
                None,
            ))
            .await
            .expect("start create_worktree agent (first)");
        assert_eq!(start_first.status(), StatusCode::OK);

        let stop_first = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/stop"),
                Some(&token),
                None,
            ))
            .await
            .expect("stop create_worktree agent (first)");
        assert_eq!(stop_first.status(), StatusCode::OK);

        let start_second = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/start"),
                Some(&token),
                None,
            ))
            .await
            .expect("start create_worktree agent (second)");
        assert_eq!(start_second.status(), StatusCode::OK);

        let stop_second = app
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/stop"),
                Some(&token),
                None,
            ))
            .await
            .expect("stop create_worktree agent (second)");
        assert_eq!(stop_second.status(), StatusCode::OK);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn start_route_with_actor_runtime_payload_injects_actor_envs() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = router(state.clone());

        let workdir = std::env::temp_dir().join(format!("agenthub-start-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&workdir).expect("create workdir");
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO safe_paths (path, created_at)
            VALUES (?1, ?2)
            "#,
        )
        .bind(workdir.to_string_lossy().to_string())
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert safe path");

        let env_file = workdir.join("actor-runtime-env.txt");
        let script = format!(
            "printf '%s\\n' \"$AGENTHUB_ACTOR_RUN_ID\" > actor-runtime-env.txt; \
             printf '%s\\n' \"$AGENTHUB_ACTOR_ID\" >> actor-runtime-env.txt; \
             printf '%s\\n' \"$AGENTHUB_ACTOR_CHANNEL\" >> actor-runtime-env.txt; \
             printf '%s\\n' \"$AGENTHUB_ACTOR_CLI\" >> actor-runtime-env.txt; \
             sleep 30"
        );

        let create_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "actor-runtime-api-agent",
                    "workdir": workdir.to_string_lossy().to_string(),
                    "command": "/bin/sh",
                    "args": ["-lc", script],
                    "code_mode": true
                })),
            ))
            .await
            .expect("create agent via router");
        assert_eq!(create_resp.status(), StatusCode::OK);
        let created = decode_json_body(create_resp).await;
        let agent_id = created["id"].as_str().expect("agent id");

        let cli_path = default_actor_cli_path().expect("resolve default cli path");
        let start_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/start"),
                Some(&token),
                Some(json!({
                    "actor_runtime": {
                        "run_id": "run-api-start",
                        "actor_id": "planner",
                        "channel": "coordination",
                        "actor_cli_path": cli_path
                    }
                })),
            ))
            .await
            .expect("start agent with actor runtime context");
        assert_eq!(start_resp.status(), StatusCode::OK);

        let mut wrote_env = false;
        for _ in 0..50 {
            if env_file.exists() {
                wrote_env = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(wrote_env, "agent did not write actor runtime env file");

        let contents = std::fs::read_to_string(&env_file).expect("read actor env file");
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 4, "unexpected actor env file format");
        assert_eq!(lines[0], "run-api-start");
        assert_eq!(lines[1], "planner");
        assert_eq!(lines[2], "coordination");
        assert_eq!(
            lines[3],
            default_actor_cli_path().expect("resolve default cli path")
        );

        let stop_resp = app
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{agent_id}/stop"),
                Some(&token),
                None,
            ))
            .await
            .expect("stop agent");
        assert_eq!(stop_resp.status(), StatusCode::OK);

        let _ = std::fs::remove_dir_all(&workdir);
    }
}
