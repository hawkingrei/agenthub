use std::path::Path as StdPath;
use std::path::PathBuf;
use std::process::Command as StdCommand;
use std::sync::Arc;
use std::sync::OnceLock;
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
use crate::acp::DEFAULT_ACTOR_CHANNEL;
use crate::agent::AgentManager;
use crate::agent::WorktreeMode;
use crate::agenthub_binary::resolve_agenthub_binary_path;
use crate::auth::AuthService;
use crate::push::PushService;
use crate::state::AppState;
use crate::team::{
    TeamActorMessageTransport, TeamDefinitionConfig, TeamManager, force_team_member_new_session,
};
use agenthub_config::{AppConfig, PushConfig, WebConfig};
use agenthub_message_archive::{
    MessageArchiveStore, MessageDocument, MessageDocumentKind, MessageSearchHit, MessageSearchQuery,
};
use agenthub_team_actor::{
    ActorAckRequest, ActorInboxRequest, ActorMailboxService, ActorSendRequest,
};
use async_trait::async_trait;

use super::{
    AckTeamRunMessageRequest, CompileTeamTaskRunPreviewRequest, CompleteTeamRunStepRequest,
    CreateTeamChannelRequest, CreateTeamRequest, CreateTeamRunRequest, CreateTeamTaskRequest,
    FailTeamRunStepRequest, FlushTeamRunContextRequest, ListTeamRunEventsQuery,
    ListTeamRunInboxQuery, ListTeamRunsQuery, ListTeamTaskMessagesQuery, ListTeamTasksQuery,
    ReplyTeamThreadRequest, ResumeTeamRunStepRequest, SearchTeamMessagesQuery,
    SendTeamRunMessageRequest, SendTeamTaskMessageRequest, SetTeamRunStepInputRequiredRequest,
    StartTeamRunStepRequest, SubmitTeamRunStepRequest, TeamMemberSpec,
    TeamMessageSearchHitResponse, TeamRunSnapshotQuery, TeamTaskDetailResponse,
    UpdateTeamSpecRequest, UpdateTeamTaskRequest, ack_team_run_message, cancel_team_run,
    compile_team_task_run_preview, complete_team_run_step, create_team, create_team_channel,
    create_team_run, delete_team, delete_team_channel, ensure_team_shared_thread,
    fail_team_run_step, flush_team_run_context, force_new_session_for_team_member, get_team,
    get_team_run, get_team_run_snapshot, get_team_runtime, get_team_shared_thread, get_team_task,
    list_team_channels, list_team_run_events, list_team_run_inbox, list_team_run_steps,
    list_team_runs, list_team_task_messages, list_team_tasks, list_teams, load_team_for_user,
    map_team_internal_error, normalize_conversation_mode, normalize_task_created_by_actor_id,
    normalize_team_spec, parse_message_archive_source_kind, reply_team_thread, require_user,
    restart_team_run, resume_team_run, resume_team_run_step, search_team_messages,
    send_team_run_message, send_team_task_message, set_team_run_step_input_required, start_team,
    start_team_run_step, stop_team, submit_team_run_step, update_team_spec, update_team_task,
    validate_team_spec,
};

#[derive(Default)]
struct RecordingSearchArchive {
    queries: tokio::sync::Mutex<Vec<MessageSearchQuery>>,
    hits: Vec<MessageSearchHit>,
}

#[async_trait]
impl MessageArchiveStore for RecordingSearchArchive {
    async fn ensure_ready(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn append_documents(&self, _documents: &[MessageDocument]) -> anyhow::Result<()> {
        Ok(())
    }

    async fn search(&self, query: &MessageSearchQuery) -> anyhow::Result<Vec<MessageSearchHit>> {
        self.queries.lock().await.push(query.clone());
        Ok(self.hits.clone())
    }
}

static WORKER_TEST_REPO: OnceLock<String> = OnceLock::new();
static TEST_AGENTHUB_BIN: OnceLock<String> = OnceLock::new();

async fn create_team_task(
    state: &AppState,
    headers: &HeaderMap,
    team_id: &str,
    payload: CreateTeamTaskRequest,
) -> Result<TeamTaskDetailResponse, crate::api::error::ApiError> {
    let user = require_user(headers, state).await?;
    load_team_for_user(state, team_id, &user).await?;
    let title = payload.title.trim().to_string();
    if title.is_empty() {
        return Err(crate::api::error::ApiError::bad_request(
            "title is required",
        ));
    }
    let created_by_actor_id =
        normalize_task_created_by_actor_id(payload.created_by_actor_id.as_deref(), &user)?;
    let conversation_mode = normalize_conversation_mode(payload.conversation_mode.as_deref())?;
    let raw_context = payload.context.unwrap_or_else(|| json!({}));
    let (task, conversation) = state
        .teams
        .create_task(
            team_id,
            &title,
            &created_by_actor_id,
            raw_context,
            &conversation_mode,
            payload.topic.as_deref(),
        )
        .await
        .map_err(map_team_internal_error)?;
    Ok(TeamTaskDetailResponse {
        task,
        conversation,
        latest_run: None,
    })
}

fn build_team_member_actor_context(
    team_id: &str,
    member: &TeamMemberSpec,
) -> anyhow::Result<AcpActorSkillContext> {
    Ok(AcpActorSkillContext {
        team_id: Some(team_id.to_string()),
        current_run_id: None,
        actor_id: member.member_id.clone(),
        default_channel: DEFAULT_ACTOR_CHANNEL.to_string(),
        member_role: Some(member.role.clone()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    })
}

fn team_member_actor_context_matches(
    current: Option<&AcpActorSkillContext>,
    expected: &AcpActorSkillContext,
) -> bool {
    let Some(current) = current else {
        return false;
    };
    current.team_id == expected.team_id
        && current.current_run_id == expected.current_run_id
        && current.actor_id == expected.actor_id
        && current.default_channel == expected.default_channel
        && current.member_role == expected.member_role
        && current.member_skills == expected.member_skills
}

fn resolve_test_agenthub_binary_path() -> String {
    TEST_AGENTHUB_BIN
        .get_or_init(|| {
            if let Some(path) = resolve_agenthub_binary_path() {
                return path.to_string_lossy().to_string();
            }

            let current = std::env::current_exe().expect("resolve current test executable path");
            panic!(
                "resolve real agenthub binary path for tests from {}",
                current.display()
            );
        })
        .clone()
}

fn long_lived_test_agent_command() -> String {
    "/bin/sh".to_string()
}

fn long_lived_test_agent_args() -> String {
    serde_json::to_string(&vec!["-c", "exec /bin/sleep 30"])
        .expect("serialize long-lived test agent args")
}

pub(crate) async fn build_test_state() -> AppState {
    build_test_state_with_db_source_and_archive(None, true, true, None).await
}

pub(crate) async fn build_test_state_without_seeded_team_member_agents() -> AppState {
    build_test_state_with_db_source_and_archive(None, true, false, None).await
}

pub(crate) async fn build_test_state_with_db_path(path: &StdPath) -> AppState {
    build_test_state_with_db_source_and_archive(Some(path), true, true, None).await
}

pub(crate) async fn reopen_test_state_with_db_path(path: &StdPath) -> AppState {
    build_test_state_with_db_source_and_archive(Some(path), false, false, None).await
}

pub(crate) async fn build_test_state_with_message_archive(
    archive: Arc<dyn MessageArchiveStore>,
) -> AppState {
    build_test_state_with_db_source_and_archive(None, true, false, Some(archive)).await
}

async fn build_test_state_with_db_source_and_archive(
    path: Option<&StdPath>,
    initialize_schema: bool,
    seed_default_agents: bool,
    message_archive: Option<Arc<dyn MessageArchiveStore>>,
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
            passkey_enabled: None,
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
    let event_dbs = agenthub_db::AgentEventDbRouter::new(
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
        true,
        permissions.clone(),
        auth.clone(),
    ));
    let teams = Arc::new(TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        event_dbs,
        message_archive,
    ));
    let state = AppState {
        db,
        agents,
        teams,
        push,
        auth,
        acp_permissions: permissions,
        agent_node_join_bootstrap: crate::agent::AgentNodeJoinBootstrapInfo::disabled(),
        default_worktree_root: config.default_worktree_root(),
    };
    if seed_default_agents {
        seed_default_team_member_agents(&state).await;
    }
    state
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
        CREATE TABLE join_challenges (
            token TEXT PRIMARY KEY,
            pin_hash TEXT NOT NULL,
            expires_at INTEGER NOT NULL,
            created_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create join_challenges");

    sqlx::query(
        r#"
        CREATE TABLE login_audit (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT,
            device_id TEXT,
            event TEXT NOT NULL,
            ip TEXT,
            user_agent TEXT,
            detail TEXT,
            ts INTEGER NOT NULL
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create login_audit");

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
            agent_loop_enabled INTEGER NOT NULL DEFAULT 0,
            agent_loop_idle_seconds INTEGER,
            agent_loop_prompt TEXT,
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
        CREATE TABLE agent_persistent_sessions (
            agent_id TEXT NOT NULL,
            provider TEXT NOT NULL,
            session_id TEXT NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (agent_id, provider),
            FOREIGN KEY(agent_id) REFERENCES agents(id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create agent_persistent_sessions");

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
            team_id TEXT,
            requester_actor_id TEXT,
            requester_role TEXT,
            review_target_actor_id TEXT,
            review_dispatch_status TEXT,
            review_delivery_run_id TEXT,
            review_message_id INTEGER,
            review_dispatched_at INTEGER,
            reviewed_by_actor_id TEXT,
            human_review_notified_at INTEGER,
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
            group_id TEXT,
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
            group_id TEXT,
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
            group_id TEXT,
            title TEXT NOT NULL,
            status TEXT NOT NULL,
            created_by_actor_id TEXT NOT NULL,
            assigned_member_id TEXT,
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
        CREATE UNIQUE INDEX idx_team_channel_bootstrap_unique
        ON team_tasks(
            team_id,
            lower(trim(COALESCE(json_extract(context_json, '$.channel_id'), '')))
        )
        WHERE lower(trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), ''))) = 'team_channel';
        "#,
    )
    .execute(db)
    .await
    .expect("create team channel bootstrap unique index");

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
            correlation_id TEXT NOT NULL DEFAULT '',
            payload_json TEXT NOT NULL,
            idempotency_key TEXT,
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
        CREATE UNIQUE INDEX idx_team_conversation_messages_idempotency
        ON team_conversation_messages(conversation_id, from_actor_id, idempotency_key)
        WHERE idempotency_key IS NOT NULL;
        "#,
    )
    .execute(db)
    .await
    .expect("create team_conversation_messages idempotency index");

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
        CREATE TABLE team_channel_message_replicas (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            authority_message_id INTEGER NOT NULL,
            correlation_id TEXT NOT NULL,
            run_id TEXT NOT NULL,
            team_id TEXT NOT NULL,
            conversation_id TEXT NOT NULL,
            task_id TEXT NOT NULL,
            channel_id TEXT NOT NULL,
            from_actor_id TEXT NOT NULL,
            source_node_id TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            stored_at INTEGER NOT NULL,
            UNIQUE(authority_message_id)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create team_channel_message_replicas");

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

const DEFAULT_TEST_TEAM_MEMBER_IDS: &[&str] = &[
    "planner",
    "reviewer",
    "coordinator",
    "coordinator-agent",
    "executor",
    "qa-review",
    "worker-1",
    "worker-2",
    "worker-agent",
    "worker-agent-a",
    "worker-agent-b",
    "worker-dev",
];

const DEFAULT_TEST_WORKER_MEMBER_IDS: &[&str] = &[
    "reviewer",
    "qa-review",
    "worker-1",
    "worker-2",
    "worker-agent",
    "worker-agent-a",
    "worker-agent-b",
    "worker-dev",
];

async fn seed_default_team_member_agents(state: &AppState) {
    let workdir =
        std::env::temp_dir().join(format!("agenthub-team-api-test-members-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&workdir).expect("create team member workdir");
    let workdir = workdir.to_string_lossy().to_string();
    let actor_cli = resolve_test_agenthub_binary_path();
    let actor_args = serde_json::to_string(&vec!["actor"]).expect("serialize actor args");
    let now = Utc::now().timestamp();
    for safe_path in [&workdir, "/tmp"] {
        sqlx::query("INSERT OR IGNORE INTO safe_paths (path, created_at) VALUES (?1, ?2)")
            .bind(safe_path)
            .bind(now)
            .execute(&state.db)
            .await
            .expect("insert safe path for team member agents");
    }

    for member_id in DEFAULT_TEST_TEAM_MEMBER_IDS {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref,
                code_mode, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9)
            "#,
        )
        .bind(member_id)
        .bind(format!("{member_id}-agent"))
        .bind(&workdir)
        .bind(&actor_cli)
        .bind(&actor_args)
        .bind("use_existing")
        .bind("created")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("seed default team member agent");
    }

    for member_id in DEFAULT_TEST_WORKER_MEMBER_IDS {
        configure_worker_team_member_agent(state, member_id).await;
    }
}

pub(crate) async fn configure_worker_team_member_agent(state: &AppState, agent_id: &str) {
    let now = Utc::now().timestamp();
    let repo = worker_test_repo().clone();
    let worktree_root = std::env::temp_dir()
        .join(format!(
            "agenthub-team-api-test-worker-worktrees-{}",
            Uuid::new_v4()
        ))
        .to_string_lossy()
        .to_string();
    std::fs::create_dir_all(&worktree_root).expect("create worker runtime root");
    let workdir = StdPath::new(&worktree_root).join(agent_id);
    if workdir.exists() {
        std::fs::remove_dir_all(&workdir).expect("clear stale worker workdir");
    }
    let workdir = workdir.to_string_lossy().to_string();
    sqlx::query("INSERT OR IGNORE INTO safe_paths (path, created_at) VALUES (?1, ?2)")
        .bind(&repo)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert repo safe path for worker team member agent");
    sqlx::query("INSERT OR IGNORE INTO safe_paths (path, created_at) VALUES (?1, ?2)")
        .bind(&worktree_root)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert worker runtime root safe path");
    sqlx::query(
        r#"
        UPDATE agents
        SET workdir = ?2,
            worktree_mode = 'create_worktree',
            worktree_repo = ?3,
            worktree_ref = 'HEAD',
            status = 'created',
            updated_at = ?4
        WHERE id = ?1
        "#,
    )
    .bind(agent_id)
    .bind(&workdir)
    .bind(repo)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("configure worker team member agent");
}

pub(crate) async fn configure_long_lived_team_member_agent(state: &AppState, agent_id: &str) {
    let now = Utc::now().timestamp();
    let command = long_lived_test_agent_command();
    let args = long_lived_test_agent_args();
    let workdir = state
        .agents
        .get_agent(agent_id)
        .await
        .expect("load long-lived team member agent")
        .workdir;
    std::fs::create_dir_all(&workdir).expect("create long-lived team member workdir");
    sqlx::query(
        r#"
        UPDATE agents
        SET command = ?2,
            args = ?3,
            worktree_mode = 'use_existing',
            worktree_repo = NULL,
            worktree_ref = NULL,
            status = 'created',
            updated_at = ?4
        WHERE id = ?1
        "#,
    )
    .bind(agent_id)
    .bind(command)
    .bind(args)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("configure long-lived team member agent");
}

pub(crate) async fn insert_legacy_team_member_agent(state: &AppState, agent_id: &str) -> String {
    let workdir = std::env::temp_dir().join(format!(
        "agenthub-team-legacy-member-{agent_id}-{}",
        Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workdir).expect("create legacy team member workdir");
    let workdir = workdir.to_string_lossy().to_string();
    let actor_cli = resolve_test_agenthub_binary_path();
    let actor_args = serde_json::to_string(&vec!["actor"]).expect("serialize actor args");
    let now = Utc::now().timestamp();
    sqlx::query("INSERT OR IGNORE INTO safe_paths (path, created_at) VALUES (?1, ?2)")
        .bind(&workdir)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert safe path for legacy team member");
    sqlx::query(
        r#"
        INSERT OR REPLACE INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref,
            code_mode, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, 'use_existing', NULL, NULL, 0, 'created', ?6, ?7)
        "#,
    )
    .bind(agent_id)
    .bind(format!("{agent_id}-agent"))
    .bind(&workdir)
    .bind(&actor_cli)
    .bind(&actor_args)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert legacy team member agent");
    workdir
}

#[test]
fn resolve_test_agenthub_binary_path_prefers_real_binary() {
    let path = resolve_test_agenthub_binary_path();
    let path_buf = PathBuf::from(&path);
    assert!(path_buf.exists(), "expected binary path to exist: {path}");
    let file_name = path_buf
        .file_name()
        .and_then(|value| value.to_str())
        .expect("binary file name");
    assert_eq!(
        file_name,
        format!("agenthub{}", std::env::consts::EXE_SUFFIX)
    );
    assert!(
        !path.contains("/deps/"),
        "expected real binary path, got test harness path: {path}"
    );
}

fn worker_test_repo() -> &'static String {
    WORKER_TEST_REPO.get_or_init(|| {
        let base =
            std::env::temp_dir().join(format!("agenthub-team-worker-repo-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&base).expect("create worker test repo dir");
        run_git(&base, &["init"]);
        run_git(
            &base,
            &["config", "user.email", "agenthub-test@example.com"],
        );
        run_git(&base, &["config", "user.name", "AgentHub Test"]);
        std::fs::write(base.join("README.md"), "seed\n").expect("write worker repo seed");
        run_git(&base, &["add", "README.md"]);
        run_git(&base, &["commit", "-m", "init"]);
        base.to_string_lossy().to_string()
    })
}

fn create_named_worker_test_repo(name: &str) -> String {
    let base = std::env::temp_dir().join(format!("agenthub-team-worker-root-{}", Uuid::new_v4()));
    let repo = base.join(name);
    std::fs::create_dir_all(&repo).expect("create named worker test repo dir");
    run_git(&repo, &["init"]);
    run_git(
        &repo,
        &["config", "user.email", "agenthub-test@example.com"],
    );
    run_git(&repo, &["config", "user.name", "AgentHub Test"]);
    std::fs::write(repo.join("README.md"), format!("{name}\n"))
        .expect("write named worker repo seed");
    run_git(&repo, &["add", "README.md"]);
    run_git(&repo, &["commit", "-m", "init"]);
    repo.to_string_lossy().to_string()
}

fn run_git(repo_dir: &StdPath, args: &[&str]) {
    let status = StdCommand::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(args)
        .status()
        .expect("failed to execute `git`; ensure it is available on PATH");
    assert!(status.success(), "git command failed: {:?}", args);
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
            command: "/bin/sh".to_string(),
            args: vec![
                "-c".to_string(),
                "printf 'ACTOR_ENV_SNAPSHOT|team=%s|current_run=%s|actor=%s|channel=%s\\n' \"$AGENTHUB_ACTOR_TEAM_ID\" \"$AGENTHUB_ACTOR_CURRENT_RUN_ID\" \"$AGENTHUB_ACTOR_ID\" \"$AGENTHUB_ACTOR_CHANNEL\"".to_string(),
            ],
            target_node_id: None,
            worktree_mode: WorktreeMode::UseExisting,
            worktree_repo: None,
            worktree_ref: None,
            code_mode: true,
            agent_loop_enabled: false,
            agent_loop_idle_seconds: None,
            agent_loop_prompt: None,
        })
        .await
        .expect("create agent");

    let actor_context = AcpActorSkillContext {
        team_id: Some("team-env-check".to_string()),
        current_run_id: Some("run-env-check".to_string()),
        actor_id: "planner".to_string(),
        default_channel: "coordination".to_string(),
        member_role: Some("coordinator".to_string()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    };

    let session_id = state
        .agents
        .start_agent_with_actor_context(&agent.id, Some(actor_context))
        .await
        .expect("start agent with actor context");

    let mut actor_env_snapshot = None;

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
                let line = event.message.trim();
                if line.starts_with("ACTOR_ENV_SNAPSHOT|") {
                    actor_env_snapshot = Some(line.to_string());
                }
            }
            if actor_env_snapshot.is_some() {
                break;
            }
            before_id = events.first().map(|event| event.event_id);
        }
        if actor_env_snapshot.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let snapshot = actor_env_snapshot.unwrap_or_default();
    assert!(
        snapshot.contains("team=team-env-check"),
        "missing AGENTHUB_ACTOR_TEAM_ID in process env output: {snapshot}"
    );
    assert!(
        snapshot.contains("current_run=run-env-check"),
        "missing AGENTHUB_ACTOR_CURRENT_RUN_ID in process env output: {snapshot}"
    );
    assert!(
        snapshot.contains("actor=planner"),
        "missing AGENTHUB_ACTOR_ID in process env output: {snapshot}"
    );
    assert!(
        snapshot.contains("channel=coordination"),
        "missing AGENTHUB_ACTOR_CHANNEL in process env output: {snapshot}"
    );
    let _ = std::fs::remove_dir_all(&workdir);
}
