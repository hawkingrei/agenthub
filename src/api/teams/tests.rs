use std::path::Path as StdPath;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::body::{Body, to_bytes};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, Method, Request, StatusCode, header};
use axum::response::IntoResponse;
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use tower::util::ServiceExt;
use uuid::Uuid;

use crate::acp::AcpActorSkillContext;
use crate::acp::AcpPermissionService;
use crate::acp::default_actor_cli_path;
use crate::agent::AgentManager;
use crate::agent::WorktreeMode;
use crate::auth::AuthService;
use crate::config::{AppConfig, PushConfig, WebConfig};
use crate::push::PushService;
use crate::state::AppState;
use crate::team::TeamManager;

use super::{
    AckTeamRunMessageRequest, CompileTeamTaskRunPreviewRequest, CompleteTeamRunStepRequest,
    CreateTeamRequest, CreateTeamRunRequest, CreateTeamTaskRequest, FailTeamRunStepRequest,
    FlushTeamRunContextRequest, ListTeamRunEventsQuery, ListTeamRunInboxQuery, ListTeamRunsQuery,
    ListTeamTaskMessagesQuery, ListTeamTasksQuery, ResumeTeamRunStepRequest,
    SendTeamRunMessageRequest, SendTeamTaskMessageRequest, SetTeamRunStepInputRequiredRequest,
    StartTeamRunStepRequest, SubmitTeamRunStepRequest, TeamRunSnapshotQuery, ack_team_run_message,
    cancel_team_run, compile_team_task_run_preview, complete_team_run_step, create_team,
    create_team_run, create_team_task, delete_team, fail_team_run_step, flush_team_run_context,
    get_team, get_team_run, get_team_run_snapshot, get_team_task, list_team_run_events,
    list_team_run_inbox, list_team_run_steps, list_team_runs, list_team_task_messages,
    list_team_tasks, list_teams, restart_team_run, resume_team_run, resume_team_run_step,
    send_team_run_message, send_team_task_message, set_team_run_step_input_required,
    start_team_run_step, submit_team_run_step,
};

pub(crate) async fn build_test_state() -> AppState {
    build_test_state_with_db_source(None, true).await
}

pub(crate) async fn build_test_state_with_db_path(path: &StdPath) -> AppState {
    build_test_state_with_db_source(Some(path), true).await
}

pub(crate) async fn reopen_test_state_with_db_path(path: &StdPath) -> AppState {
    build_test_state_with_db_source(Some(path), false).await
}

async fn build_test_state_with_db_source(
    path: Option<&StdPath>,
    initialize_schema: bool,
) -> AppState {
    let db = match path {
        Some(path) => create_test_db_at(path).await,
        None => create_test_db().await,
    };
    if initialize_schema {
        init_test_schema(&db).await;
    }
    let keys_dir = std::env::temp_dir().join(format!("agenthub-a2a-{}", Uuid::new_v4()));
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
            .expect("create auth"),
    );
    let permissions = Arc::new(AcpPermissionService::new(db.clone()));
    let event_dbs = crate::db::AgentEventDbRouter::new(
        std::env::temp_dir().join(format!("agenthub-api-teams-eventdb-{}", Uuid::new_v4())),
    );
    let agents = Arc::new(AgentManager::new(
        db.clone(),
        event_dbs.clone(),
        None,
        push.clone(),
        Vec::new(),
        "agenthub-codex-acp".to_string(),
        None,
        permissions.clone(),
        auth.clone(),
    ));
    let teams = Arc::new(TeamManager::new_with_event_dbs(db.clone(), event_dbs));
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

async fn create_test_db_at(path: &StdPath) -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(path)
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
        );
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
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create auth_sessions");

    sqlx::query(
        r#"
        CREATE TABLE devices (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            name TEXT NOT NULL,
            user_agent TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            last_login_at INTEGER,
            FOREIGN KEY(user_id) REFERENCES users(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create devices");

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
        );
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
        );
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
        );
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
            message BLOB NOT NULL,
            FOREIGN KEY(agent_id) REFERENCES agents(id),
            FOREIGN KEY(session_id) REFERENCES agent_sessions(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create agent_events");

    sqlx::query(
        r#"
        CREATE TABLE acp_permission_requests (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            session_id TEXT NOT NULL,
            acp_session_id TEXT,
            tool_call_id TEXT,
            options_json TEXT NOT NULL,
            tool_call_json TEXT,
            status TEXT NOT NULL,
            selected_option_id TEXT,
            created_at INTEGER NOT NULL,
            responded_at INTEGER,
            FOREIGN KEY(agent_id) REFERENCES agents(id),
            FOREIGN KEY(session_id) REFERENCES agent_sessions(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create acp_permission_requests");

    sqlx::query(
        r#"
        CREATE TABLE team_definitions (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            description TEXT,
            spec_json TEXT NOT NULL,
            owner_user_id TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create team_definitions");

    sqlx::query(
        r#"
        CREATE TABLE team_runs (
            id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL,
            context_id TEXT NOT NULL,
            status TEXT NOT NULL,
            input_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            started_at INTEGER,
            ended_at INTEGER,
            FOREIGN KEY(team_id) REFERENCES team_definitions(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create team_runs");

    sqlx::query(
        r#"
        CREATE TABLE team_steps (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            step_key TEXT NOT NULL,
            member_id TEXT NOT NULL,
            remote_task_id TEXT,
            status TEXT NOT NULL,
            attempt INTEGER NOT NULL DEFAULT 0,
            depends_on_json TEXT NOT NULL DEFAULT '[]',
            input_json TEXT,
            output_json TEXT,
            error_text TEXT,
            started_at INTEGER,
            ended_at INTEGER,
            UNIQUE(run_id, step_key, attempt),
            FOREIGN KEY(run_id) REFERENCES team_runs(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create team_steps");

    sqlx::query(
        r#"
        CREATE TABLE team_run_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT NOT NULL,
            step_id TEXT,
            event_type TEXT NOT NULL,
            ts INTEGER NOT NULL,
            payload_json TEXT NOT NULL,
            FOREIGN KEY(run_id) REFERENCES team_runs(id),
            FOREIGN KEY(step_id) REFERENCES team_steps(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create team_run_events");

    sqlx::query(
        r#"
        CREATE TABLE team_tasks (
            id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL,
            title TEXT NOT NULL,
            status TEXT NOT NULL,
            created_by_actor_id TEXT NOT NULL,
            context_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY(team_id) REFERENCES team_definitions(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create team_tasks");

    sqlx::query(
        r#"
        CREATE TABLE team_conversations (
            id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL,
            task_id TEXT NOT NULL UNIQUE,
            mode TEXT NOT NULL,
            topic TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY(team_id) REFERENCES team_definitions(id),
            FOREIGN KEY(task_id) REFERENCES team_tasks(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create team_conversations");

    sqlx::query(
        r#"
        CREATE TABLE team_conversation_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id TEXT NOT NULL,
            task_id TEXT NOT NULL,
            from_actor_id TEXT NOT NULL,
            to_actor_id TEXT,
            route TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY(conversation_id) REFERENCES team_conversations(id),
            FOREIGN KEY(task_id) REFERENCES team_tasks(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create team_conversation_messages");

    sqlx::query(
        r#"
        CREATE TABLE team_actor_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT NOT NULL,
            from_actor_id TEXT NOT NULL,
            from_peer_id TEXT NOT NULL DEFAULT 'main',
            to_actor_id TEXT NOT NULL,
            to_peer_id TEXT NOT NULL DEFAULT 'main',
            channel TEXT NOT NULL,
            transport TEXT NOT NULL,
            route_json TEXT,
            payload_json TEXT NOT NULL,
            idempotency_key TEXT,
            status TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            delivered_at INTEGER,
            relay_attempt INTEGER NOT NULL DEFAULT 0,
            relay_next_retry_at INTEGER,
            relay_last_error TEXT,
            dead_letter_at INTEGER,
            FOREIGN KEY(run_id) REFERENCES team_runs(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create team_actor_messages");

    sqlx::query(
        r#"
        CREATE UNIQUE INDEX idx_team_actor_messages_idempotency
        ON team_actor_messages(run_id, from_actor_id, from_peer_id, idempotency_key)
        WHERE idempotency_key IS NOT NULL
        "#,
    )
    .execute(db)
    .await
    .expect("create team_actor_messages idempotency index");

    sqlx::query(
        r#"
        CREATE TABLE team_member_continuity_state (
            team_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            source_run_id TEXT NOT NULL,
            source_session_id TEXT,
            summary_text TEXT NOT NULL,
            history_window_json TEXT NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (team_id, member_id),
            FOREIGN KEY(team_id) REFERENCES team_definitions(id),
            FOREIGN KEY(source_run_id) REFERENCES team_runs(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create team_member_continuity_state");

    sqlx::query(
        r#"
        CREATE TABLE team_context_artifacts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            team_id TEXT NOT NULL,
            run_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            session_id TEXT,
            artifact_seq INTEGER NOT NULL,
            artifact_kind TEXT NOT NULL,
            artifact_path TEXT NOT NULL,
            artifact_size_bytes INTEGER NOT NULL,
            content_checksum TEXT,
            created_at INTEGER NOT NULL,
            FOREIGN KEY(team_id) REFERENCES team_definitions(id),
            FOREIGN KEY(run_id) REFERENCES team_runs(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create team_context_artifacts");

    sqlx::query(
        r#"
        CREATE TABLE team_context_flush_checkpoint (
            team_id TEXT NOT NULL,
            run_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            session_id TEXT NOT NULL,
            last_event_id INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (run_id, member_id, session_id),
            FOREIGN KEY(team_id) REFERENCES team_definitions(id),
            FOREIGN KEY(run_id) REFERENCES team_runs(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create team_context_flush_checkpoint");
}

async fn auth_headers(state: &AppState) -> HeaderMap {
    let token = create_auth_token(state).await;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).expect("auth header"),
    );
    headers
}

pub(crate) async fn create_auth_token(state: &AppState) -> String {
    let user_id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();
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
        .expect("create token")
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
        Some(value) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(value.to_string()))
            .expect("build json request"),
        None => builder.body(Body::empty()).expect("build request"),
    }
}

async fn decode_json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    serde_json::from_slice(&bytes).expect("decode json response")
}

include!("tests_core.rs");
include!("tests_router.rs");

#[tokio::test]
async fn start_agent_with_actor_context_injects_runtime_env_vars() {
    let state = build_test_state().await;
    let workdir = std::env::temp_dir().join(format!("agenthub-actor-env-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&workdir).expect("create workdir");
    let workdir_str = workdir.to_string_lossy().to_string();
    let now = Utc::now().timestamp();

    sqlx::query("INSERT INTO safe_paths (path, created_at) VALUES (?1, ?2)")
        .bind(&workdir_str)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert safe path");

    let agent = state
        .agents
        .create_agent(crate::agent::AgentConfig {
            name: format!("actor-env-{}", Uuid::new_v4()),
            workdir: workdir_str.clone(),
            command: "env".to_string(),
            args: vec![],
            worktree_mode: WorktreeMode::UseExisting,
            worktree_repo: None,
            worktree_ref: None,
            code_mode: true,
        })
        .await
        .expect("create agent");

    let actor_cli_path = default_actor_cli_path().expect("resolve actor cli path");
    let actor_context = AcpActorSkillContext {
        run_id: "run-env-check".to_string(),
        actor_id: "planner".to_string(),
        default_channel: "coordination".to_string(),
        actor_cli_path: actor_cli_path.clone(),
        member_role: Some("leader".to_string()),
        member_skills: Vec::new(),
        continuity: None,
    };

    let session_id = state
        .agents
        .start_agent_with_actor_context(&agent.id, Some(actor_context))
        .await
        .expect("start agent with actor context");

    let mut has_run_id = false;
    let mut has_actor_id = false;
    let mut has_channel = false;
    let mut has_actor_cli = false;

    for _ in 0..40 {
        let mut before_id = None;
        loop {
            let events = state
                .agents
                .list_events_for_session(&agent.id, &session_id, 2000, before_id)
                .await
                .expect("list events");
            if events.is_empty() {
                break;
            }
            for event in &events {
                let line = event.message.as_str();
                if line == "AGENTHUB_ACTOR_RUN_ID=run-env-check" {
                    has_run_id = true;
                } else if line == "AGENTHUB_ACTOR_ID=planner" {
                    has_actor_id = true;
                } else if line == "AGENTHUB_ACTOR_CHANNEL=coordination" {
                    has_channel = true;
                } else if line.starts_with("AGENTHUB_ACTOR_CLI=") && line.ends_with(&actor_cli_path)
                {
                    has_actor_cli = true;
                }
            }
            if has_run_id && has_actor_id && has_channel && has_actor_cli {
                break;
            }
            before_id = events.first().map(|event| event.event_id);
        }
        if has_run_id && has_actor_id && has_channel && has_actor_cli {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    assert!(
        has_run_id,
        "missing AGENTHUB_ACTOR_RUN_ID in process env output"
    );
    assert!(
        has_actor_id,
        "missing AGENTHUB_ACTOR_ID in process env output"
    );
    assert!(
        has_channel,
        "missing AGENTHUB_ACTOR_CHANNEL in process env output"
    );
    assert!(
        has_actor_cli,
        "missing AGENTHUB_ACTOR_CLI in process env output"
    );

    let _ = std::fs::remove_dir_all(&workdir);
}
