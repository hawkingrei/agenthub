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
use crate::object_upload::ObjectUploadService;
use crate::push::PushService;
use crate::state::AppState;
use crate::team::{
    TeamActorMessageTransport, TeamDefinitionConfig, TeamManager, TeamTaskCreateInput,
    TeamTaskPriority, TeamTaskStatus, force_team_member_new_session,
};
use agenthub_auth_domain::UserRole;
use agenthub_config::{AppConfig, ObjectStoreConfig, PushConfig, WebConfig};
use agenthub_message_archive::{
    MessageArchiveStore, MessageDocument, MessageDocumentKind, MessageSearchHit, MessageSearchQuery,
};
use agenthub_team_actor::{
    ACTOR_NODE_PEER_ID, ActorAckRequest, ActorInboxRequest, ActorMailboxService, ActorSendRequest,
};
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};

use crate::api::authz::require_user;

use super::{
    AcceptTeamspaceInviteRequest, AckTeamRunMessageRequest, CompileTeamTaskRunPreviewRequest,
    CompleteGoalForkRequest, CompleteTeamRunStepRequest, CreateGoalForkRequest,
    CreateTeamChannelRequest, CreateTeamRequest, CreateTeamRunRequest, CreateTeamTaskRequest,
    CreateTeamspaceInviteRequest, EscalateTeamRunMessageRequest, FailTeamRunStepRequest,
    FlushTeamRunContextRequest, ListTeamRunEventsQuery, ListTeamRunInboxQuery, ListTeamRunsQuery,
    ListTeamTaskMessagesQuery, ListTeamTasksQuery, ReplyTeamThreadRequest,
    ResumeTeamRunStepRequest, SearchTeamMessagesQuery, SendTeamRunMessageRequest,
    SendTeamTaskMessageRequest, SetTeamRunStepInputRequiredRequest, StartTeamRunStepRequest,
    SubmitTeamRunStepRequest, TakeoverTeamRunMessageRequest, TeamDownloadRequest, TeamMemberSpec,
    TeamMessageSearchHitResponse, TeamRunSnapshotQuery, TeamTaskDetailResponse, TeamUploadRequest,
    TransferTeamRunMessageRequest, TriageTeamRunMessageRequest, UpdateTeamSpecRequest,
    UpdateTeamTaskRequest, accept_teamspace_invite, ack_team_run_message, cancel_team_run,
    compile_team_task_run_preview, complete_goal_fork, complete_team_run_step, create_goal_fork,
    create_team, create_team_channel, create_team_run, create_teamspace_invite, delete_team,
    delete_team_channel, download_team_object, download_team_task_object,
    ensure_team_shared_thread, escalate_team_run_message, fail_team_run_step,
    flush_team_run_context, force_new_session_for_team_member, get_team, get_team_run,
    get_team_run_snapshot, get_team_runtime, get_team_shared_thread, get_team_task, hex_encode,
    list_goal_forks, list_team_channels, list_team_run_events, list_team_run_inbox,
    list_team_run_steps, list_team_runs, list_team_task_messages, list_team_tasks, list_teams,
    load_team_for_user, map_team_internal_error, normalize_conversation_mode,
    normalize_task_created_by_actor_id, normalize_team_spec, parse_message_archive_source_kind,
    reply_team_thread, restart_team_run, resume_team_run, resume_team_run_step,
    revoke_teamspace_member, search_team_messages, send_team_run_message, send_team_task_message,
    set_team_run_step_input_required, start_team, start_team_run_step, stop_team,
    submit_team_run_step, takeover_team_run_message, transfer_team_run_message,
    triage_team_run_message, update_team_spec, update_team_task, upload_team_image,
    upload_team_object, upload_team_task_image, upload_team_task_object, validate_team_spec,
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

    async fn contains_document(&self, document_id: &str) -> anyhow::Result<bool> {
        Ok(self
            .hits
            .iter()
            .any(|hit| hit.document_id.as_str() == document_id))
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
    let priority_raw = payload.priority.trim();
    if priority_raw.is_empty() {
        return Err(crate::api::error::ApiError::bad_request(
            "priority is required",
        ));
    }
    let priority = priority_raw.parse::<TeamTaskPriority>().map_err(|_| {
        crate::api::error::ApiError::bad_request(
            "invalid task priority; expected one of: critical, high, medium, low",
        )
    })?;
    let assigned_member_id = payload.assigned_member_id.trim();
    if assigned_member_id.is_empty() {
        return Err(crate::api::error::ApiError::bad_request(
            "assigned_member_id is required",
        ));
    }
    let created_by_actor_id =
        normalize_task_created_by_actor_id(payload.created_by_actor_id.as_deref(), &user)?;
    let conversation_mode = normalize_conversation_mode(payload.conversation_mode.as_deref())?;
    let raw_context = payload.context.unwrap_or_else(|| json!({}));
    let (task, conversation) = state
        .teams
        .create_task_with_metadata(TeamTaskCreateInput {
            team_id,
            title: &title,
            created_by_actor_id: &created_by_actor_id,
            priority,
            assigned_member_id: Some(assigned_member_id),
            context: raw_context,
            conversation_mode: &conversation_mode,
            topic: payload.topic.as_deref(),
        })
        .await
        .map_err(map_team_internal_error)?;
    Ok(TeamTaskDetailResponse {
        task,
        conversation,
        latest_run: None,
        notes: Vec::new(),
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
    build_test_state_with_db_source_archive_and_object_store(None, true, true, None, None, None)
        .await
}

pub(crate) async fn build_test_state_without_seeded_team_member_agents() -> AppState {
    build_test_state_with_db_source_archive_and_object_store(None, true, false, None, None, None)
        .await
}

pub(crate) async fn build_test_state_with_db_path(path: &StdPath) -> AppState {
    build_test_state_with_db_source_archive_and_object_store(
        Some(path),
        true,
        true,
        None,
        None,
        None,
    )
    .await
}

pub(crate) async fn reopen_test_state_with_db_path(path: &StdPath) -> AppState {
    build_test_state_with_db_source_archive_and_object_store(
        Some(path),
        false,
        false,
        None,
        None,
        None,
    )
    .await
}

pub(crate) async fn build_test_state_with_message_archive(
    archive: Arc<dyn MessageArchiveStore>,
) -> AppState {
    build_test_state_with_db_source_archive_and_object_store(
        None,
        true,
        false,
        Some(archive),
        None,
        None,
    )
    .await
}

pub(crate) async fn build_test_state_with_body_store(
    body_store: crate::message_body_store::SharedBodyStore,
) -> AppState {
    build_test_state_with_db_source_archive_and_object_store(
        None,
        true,
        true,
        None,
        Some(body_store),
        None,
    )
    .await
}

#[cfg(feature = "object-store-s3")]
async fn build_test_state_with_object_store(object_store: ObjectStoreConfig) -> AppState {
    build_test_state_with_db_source_archive_and_object_store(
        None,
        true,
        true,
        None,
        None,
        Some(object_store),
    )
    .await
}

async fn build_test_state_with_db_source_archive_and_object_store(
    path: Option<&StdPath>,
    initialize_schema: bool,
    seed_default_agents: bool,
    message_archive: Option<Arc<dyn MessageArchiveStore>>,
    body_store: Option<crate::message_body_store::SharedBodyStore>,
    object_store: Option<ObjectStoreConfig>,
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
        object_store: object_store.clone(),
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
    let teams = Arc::new(
        TeamManager::new_with_event_dbs_and_message_archive(db.clone(), event_dbs, message_archive)
            .with_body_store(body_store.clone())
            .with_message_index(None)
            .with_read_repair_scheduler(None),
    );
    let object_uploads = Arc::new(match object_store {
        Some(object_store) => {
            let config = AppConfig {
                object_store: Some(object_store),
                ..Default::default()
            };
            ObjectUploadService::from_config(db.clone(), &config)
                .expect("create configured object upload service")
        }
        None => test_object_upload_service(db.clone()),
    });
    let state = AppState {
        db,
        linker_http: crate::linkers::AppLinkerService::default_http_client(),
        agents,
        teams,
        push,
        auth,
        acp_permissions: permissions,
        object_uploads,
        agent_node_join_bootstrap: crate::agent::AgentNodeJoinBootstrapInfo::disabled(),
        default_worktree_root: config.default_worktree_root(),
        body_store,
        message_index: None,
        message_read_repair: None,
    };
    if seed_default_agents {
        seed_default_team_member_agents(&state).await;
    }
    state
}

#[cfg(feature = "object-store-s3")]
fn s3_fixture_object_store_config_from_env() -> Option<ObjectStoreConfig> {
    let endpoint = std::env::var("AGENTHUB_OBJECT_STORE_S3_TEST_ENDPOINT").ok()?;
    let bucket = std::env::var("AGENTHUB_OBJECT_STORE_S3_TEST_BUCKET").ok()?;
    std::env::var("AGENTHUB_OBJECT_STORE_S3_TEST_ACCESS_KEY_ID").ok()?;
    std::env::var("AGENTHUB_OBJECT_STORE_S3_TEST_SECRET_ACCESS_KEY").ok()?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("read system time")
        .as_nanos();
    Some(ObjectStoreConfig {
        backend: Some("s3".to_string()),
        root: None,
        prefix: Some(format!("agenthub-api-s3-ci/{nonce}")),
        public_base_url: Some("https://img.example.test/objects".to_string()),
        bucket: Some(bucket),
        endpoint: Some(endpoint),
        region: Some(
            std::env::var("AGENTHUB_OBJECT_STORE_S3_TEST_REGION")
                .unwrap_or_else(|_| "us-east-1".to_string()),
        ),
        access_key_id_env: Some("AGENTHUB_OBJECT_STORE_S3_TEST_ACCESS_KEY_ID".to_string()),
        secret_access_key_env: Some("AGENTHUB_OBJECT_STORE_S3_TEST_SECRET_ACCESS_KEY".to_string()),
        download_max_bytes: Some(1024 * 1024),
        download_max_redirects: Some(3),
        download_timeout_seconds: Some(10),
        download_retry_attempts: Some(1),
        download_retry_backoff_millis: Some(0),
        download_max_concurrent_per_host: Some(4),
        download_allow_private_networks: Some(true),
        download_allowed_hosts: None,
        download_denied_hosts: None,
    })
}

fn test_object_upload_service(db: SqlitePool) -> ObjectUploadService {
    let root = std::env::temp_dir()
        .join(format!("agenthub-test-objects-{}", Uuid::new_v4()))
        .to_string_lossy()
        .to_string();
    let config = AppConfig {
        object_store: Some(agenthub_config::ObjectStoreConfig {
            backend: Some("fs".to_string()),
            root: Some(root),
            prefix: None,
            public_base_url: None,
            bucket: None,
            endpoint: None,
            region: None,
            access_key_id_env: None,
            secret_access_key_env: None,
            download_max_bytes: Some(1024 * 1024),
            download_max_redirects: Some(3),
            download_timeout_seconds: Some(10),
            download_retry_attempts: Some(1),
            download_retry_backoff_millis: Some(0),
            download_max_concurrent_per_host: Some(4),
            download_allow_private_networks: Some(true),
            download_allowed_hosts: None,
            download_denied_hosts: None,
        }),
        ..Default::default()
    };
    ObjectUploadService::from_config(db, &config).expect("create object upload service")
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
        CREATE TABLE push_subscriptions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            endpoint TEXT NOT NULL,
            p256dh TEXT NOT NULL,
            auth TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create push_subscriptions");

    sqlx::query(
        r#"
        CREATE TABLE object_uploads (
            id TEXT PRIMARY KEY,
            owner_scope TEXT NOT NULL,
            backend TEXT NOT NULL,
            object_key TEXT NOT NULL,
            original_filename TEXT NOT NULL,
            content_type TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            sha256 TEXT NOT NULL,
            public_url TEXT,
            created_by_actor_id TEXT NOT NULL,
            publish_state TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            published_at INTEGER,
            cleanup_after INTEGER
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create object_uploads");

    sqlx::query(
        r#"
        CREATE UNIQUE INDEX idx_object_uploads_object_key
        ON object_uploads(object_key);
        "#,
    )
    .execute(db)
    .await
    .expect("create object upload object key index");

    sqlx::query(
        r#"
        CREATE TABLE object_download_metrics (
            backend TEXT PRIMARY KEY,
            attempts_total INTEGER NOT NULL DEFAULT 0,
            successes_total INTEGER NOT NULL DEFAULT 0,
            failures_total INTEGER NOT NULL DEFAULT 0,
            downloaded_bytes_total INTEGER NOT NULL DEFAULT 0,
            latency_ms_total INTEGER NOT NULL DEFAULT 0,
            latency_ms_max INTEGER NOT NULL DEFAULT 0,
            cleanup_attempts_total INTEGER NOT NULL DEFAULT 0,
            cleanup_successes_total INTEGER NOT NULL DEFAULT 0,
            cleanup_failures_total INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create object_download_metrics");

    sqlx::query(
        r#"
        CREATE TABLE object_download_failure_metrics (
            backend TEXT NOT NULL,
            failure_class TEXT NOT NULL,
            failures_total INTEGER NOT NULL DEFAULT 0,
            last_failure_at INTEGER NOT NULL,
            PRIMARY KEY(backend, failure_class),
            FOREIGN KEY(backend) REFERENCES object_download_metrics(backend) ON DELETE CASCADE
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create object_download_failure_metrics");

    sqlx::query(
        r#"
        CREATE TABLE object_upload_sessions (
            id TEXT PRIMARY KEY,
            owner_scope TEXT NOT NULL,
            backend TEXT NOT NULL,
            object_key TEXT NOT NULL,
            original_filename TEXT NOT NULL,
            content_type TEXT NOT NULL,
            object_kind TEXT NOT NULL,
            expected_size_bytes INTEGER NOT NULL,
            expected_sha256 TEXT,
            created_by_actor_id TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            expires_at INTEGER NOT NULL,
            completed_at INTEGER,
            canceled_at INTEGER,
            published_upload_id TEXT
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create object_upload_sessions");

    sqlx::query(
        r#"
        CREATE TABLE object_upload_session_parts (
            session_id TEXT NOT NULL,
            part_number INTEGER NOT NULL,
            object_key TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            sha256 TEXT NOT NULL,
            uploaded_at INTEGER NOT NULL,
            PRIMARY KEY(session_id, part_number)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create object_upload_session_parts");

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
            target_node_id TEXT,
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
            priority TEXT NOT NULL DEFAULT 'medium',
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
        CREATE TABLE team_members (
            team_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            role TEXT NOT NULL,
            created_by_user_id TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            revoked_at INTEGER,
            PRIMARY KEY (team_id, user_id)
        );
        CREATE TABLE team_invites (
            id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL,
            token_digest TEXT NOT NULL UNIQUE,
            role TEXT NOT NULL,
            created_by_user_id TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            expires_at INTEGER NOT NULL,
            accepted_by_user_id TEXT,
            accepted_at INTEGER,
            revoked_at INTEGER
        );
        CREATE TABLE team_execution_claims (
            entity_kind TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            team_id TEXT NOT NULL,
            owner_member_id TEXT NOT NULL,
            lease_generation INTEGER NOT NULL CHECK(lease_generation > 0),
            claimed_at INTEGER NOT NULL,
            expires_at INTEGER NOT NULL,
            released_at INTEGER,
            PRIMARY KEY (entity_kind, entity_id)
        );
        CREATE TABLE team_goal_leases (
            task_id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL,
            owner_member_id TEXT NOT NULL,
            lease_generation INTEGER NOT NULL,
            started_at INTEGER NOT NULL,
            expires_at INTEGER NOT NULL,
            released_at INTEGER,
            release_reason TEXT,
            CHECK(expires_at > started_at)
        );
        CREATE TABLE team_goal_forks (
            id TEXT PRIMARY KEY,
            parent_task_id TEXT NOT NULL,
            parent_lease_generation INTEGER NOT NULL CHECK(parent_lease_generation > 0),
            question TEXT NOT NULL,
            acceptance_criteria TEXT NOT NULL,
            result_schema_json TEXT NOT NULL,
            profile TEXT NOT NULL CHECK(profile = 'read_only'),
            created_at INTEGER NOT NULL,
            expires_at INTEGER NOT NULL,
            completed_at INTEGER,
            result_json TEXT,
            CHECK(expires_at > created_at),
            CHECK(
                (completed_at IS NULL AND result_json IS NULL)
                OR (completed_at IS NOT NULL AND result_json IS NOT NULL)
            )
        );
        CREATE TABLE team_invite_registration_intents (
            challenge_id TEXT PRIMARY KEY,
            invite_id TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE team_audit_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            team_id TEXT NOT NULL,
            actor_user_id TEXT,
            event_kind TEXT NOT NULL,
            subject_kind TEXT NOT NULL,
            subject_id TEXT NOT NULL,
            detail_json TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create Teamspace control-plane tables");

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
            group_id TEXT,
            payload_json TEXT NOT NULL,
            idempotency_key TEXT,
            created_at INTEGER NOT NULL,
            text TEXT,
            kind TEXT,
            thread_root_message_id INTEGER,
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
        CREATE TABLE message_body_outbox (
            authority_message_id TEXT PRIMARY KEY,
            body BLOB NOT NULL,
            staged_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create message_body_outbox");

    sqlx::query(
        r#"
        CREATE INDEX idx_message_body_outbox_staged_at
        ON message_body_outbox(staged_at);
        "#,
    )
    .execute(db)
    .await
    .expect("create message_body_outbox staged_at index");

    sqlx::query(
        r#"
        CREATE TABLE message_body_backfill_checkpoint (
            scope TEXT PRIMARY KEY,
            last_message_id INTEGER NOT NULL
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create message_body_backfill_checkpoint");

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
            message_kind TEXT NOT NULL DEFAULT 'coordination_request',
            group_id TEXT,
            idempotency_key TEXT,
            status TEXT NOT NULL,
            handling_disposition TEXT NOT NULL DEFAULT 'untriaged',
            handled_by_actor_id TEXT,
            handled_at INTEGER,
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
        CREATE TABLE team_actor_thread_claims (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT NOT NULL,
            topic_key TEXT NOT NULL,
            task_id TEXT,
            root_message_id INTEGER,
            owner_actor_id TEXT NOT NULL,
            claim_status TEXT NOT NULL,
            claimed_message_id INTEGER,
            claimed_at INTEGER NOT NULL,
            lease_expires_at INTEGER,
            updated_at INTEGER NOT NULL,
            UNIQUE(run_id, topic_key)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create team_actor_thread_claims");

    sqlx::query(
        r#"
        CREATE TABLE team_actor_message_links (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT NOT NULL,
            message_id INTEGER NOT NULL,
            task_id TEXT NOT NULL,
            relation TEXT NOT NULL,
            created_by_actor_id TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            UNIQUE(run_id, message_id, task_id, relation)
        );
        "#,
    )
    .execute(db)
    .await
    .expect("create team_actor_message_links");

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
            group_id TEXT,
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
    auth_headers_for_token(&token)
}

fn auth_headers_for_token(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).expect("auth header"),
    );
    headers
}

pub(crate) async fn create_auth_token(state: &AppState) -> String {
    create_auth_token_with_role(state, UserRole::Root).await
}

pub(crate) async fn create_auth_token_with_role(state: &AppState, role: UserRole) -> String {
    let (_user_id, token) = create_auth_token_with_role_and_user_id(state, role).await;
    token
}

pub(crate) async fn create_auth_token_with_role_and_user_id(
    state: &AppState,
    role: UserRole,
) -> (String, String) {
    let user_id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();
    sqlx::query(
        r#"
        INSERT INTO users (id, username, display_name, role, password_hash, created_at)
        VALUES (?1, ?2, ?3, ?4, NULL, ?5)
        "#,
    )
    .bind(&user_id)
    .bind(format!("{}-{}", role.as_str(), Uuid::new_v4()))
    .bind(role.as_str())
    .bind(role.as_str())
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert user");

    if role == UserRole::Device {
        sqlx::query(
            "INSERT INTO devices (id, user_id, name, user_agent, status, created_at) VALUES (?1, ?2, ?3, ?4, 'active', ?5)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&user_id)
        .bind("Role Test Device")
        .bind("role-test")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert role device");
    }

    let token = state
        .auth
        .create_session(&user_id)
        .await
        .expect("create token");
    (user_id, token)
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

#[tokio::test]
async fn teamspace_invite_grants_member_visibility_once() {
    let state = build_test_state().await;
    let owner_token = create_auth_token(&state).await;
    let Json(team) = create_team(
        State(state.clone()),
        auth_headers_for_token(&owner_token),
        Json(CreateTeamRequest {
            name: "invite-teamspace".to_string(),
            description: None,
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create Teamspace");

    let Json(created_invite) = create_teamspace_invite(
        State(state.clone()),
        auth_headers_for_token(&owner_token),
        Path(team.id.clone()),
        Json(CreateTeamspaceInviteRequest {
            role: "observer".to_string(),
            expires_in_seconds: Some(60),
        }),
    )
    .await
    .expect("create invite");
    let token = created_invite.url.rsplit('#').next().expect("invite token");

    let member_token = create_auth_token_with_role(&state, UserRole::Operator).await;
    let Json(member) = accept_teamspace_invite(
        State(state.clone()),
        auth_headers_for_token(&member_token),
        Json(AcceptTeamspaceInviteRequest {
            token: token.to_string(),
        }),
    )
    .await
    .expect("accept invite");
    assert_eq!(member.team_id, team.id);
    assert_eq!(member.role, "observer");

    let Json(visible) = list_teams(State(state.clone()), auth_headers_for_token(&member_token))
        .await
        .expect("list visible teams");
    assert!(visible.iter().any(|candidate| candidate.id == team.id));
    assert!(
        create_teamspace_invite(
            State(state.clone()),
            auth_headers_for_token(&member_token),
            Path(team.id.clone()),
            Json(CreateTeamspaceInviteRequest {
                role: "contributor".to_string(),
                expires_in_seconds: Some(60),
            }),
        )
        .await
        .is_err()
    );
    let Json(revoked) = revoke_teamspace_member(
        State(state.clone()),
        auth_headers_for_token(&owner_token),
        Path((team.id.clone(), member.user_id.clone())),
    )
    .await
    .expect("owner revokes observer");
    assert_eq!(revoked["status"], "revoked");
    let Json(visible_after_revoke) =
        list_teams(State(state.clone()), auth_headers_for_token(&member_token))
            .await
            .expect("list teams after revocation");
    assert!(
        !visible_after_revoke
            .iter()
            .any(|candidate| candidate.id == team.id)
    );
    assert!(
        accept_teamspace_invite(
            State(state),
            auth_headers_for_token(&member_token),
            Json(AcceptTeamspaceInviteRequest {
                token: token.to_string(),
            }),
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn goal_fork_api_is_authorized_bounded_and_returns_parent_evidence() {
    let state = build_test_state().await;
    let owner_token = create_auth_token(&state).await;
    let owner_headers = auth_headers_for_token(&owner_token);
    let Json(team) = create_team(
        State(state.clone()),
        owner_headers.clone(),
        Json(CreateTeamRequest {
            name: "goal-fork-api".to_string(),
            description: None,
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create Teamspace");
    let (task, _) = state
        .teams
        .create_task_with_metadata(TeamTaskCreateInput {
            team_id: &team.id,
            title: "research task",
            created_by_actor_id: "planner",
            priority: TeamTaskPriority::Medium,
            assigned_member_id: Some("planner"),
            context: json!({}),
            conversation_mode: "group_chat",
            topic: None,
        })
        .await
        .expect("create task");
    state
        .teams
        .update_task_status(&task.id, TeamTaskStatus::InProgress)
        .await
        .expect("start task");
    state
        .teams
        .claim_task_execution(&task.id, "planner", 300)
        .await
        .expect("claim task goal");

    let Json(fork) = create_goal_fork(
        State(state.clone()),
        owner_headers.clone(),
        Path((team.id.clone(), task.id.clone())),
        Json(CreateGoalForkRequest {
            question: "Check the source".to_string(),
            acceptance_criteria: "Return cited evidence".to_string(),
            result_schema: Some(json!({"type":"object"})),
            expires_in_seconds: Some(60),
        }),
    )
    .await
    .expect("create goal fork");
    assert_eq!(fork.profile, "read_only");

    let Json(invite) = create_teamspace_invite(
        State(state.clone()),
        owner_headers.clone(),
        Path(team.id.clone()),
        Json(CreateTeamspaceInviteRequest {
            role: "observer".to_string(),
            expires_in_seconds: Some(60),
        }),
    )
    .await
    .expect("create observer invite");
    let invite_token = invite.url.rsplit('#').next().expect("invite token");
    let observer_token = create_auth_token_with_role(&state, UserRole::Operator).await;
    let observer_headers = auth_headers_for_token(&observer_token);
    let Json(observer) = accept_teamspace_invite(
        State(state.clone()),
        observer_headers.clone(),
        Json(AcceptTeamspaceInviteRequest {
            token: invite_token.to_string(),
        }),
    )
    .await
    .expect("accept observer invite");
    assert_eq!(observer.role, "observer");
    let Json(observer_visible) = list_goal_forks(
        State(state.clone()),
        observer_headers.clone(),
        Path((team.id.clone(), task.id.clone())),
    )
    .await
    .expect("observer lists goal forks");
    assert_eq!(observer_visible, vec![fork.clone()]);
    let observer_create = create_goal_fork(
        State(state.clone()),
        observer_headers.clone(),
        Path((team.id.clone(), task.id.clone())),
        Json(CreateGoalForkRequest {
            question: "Unauthorized mutation".to_string(),
            acceptance_criteria: "Must be rejected".to_string(),
            result_schema: None,
            expires_in_seconds: Some(60),
        }),
    )
    .await
    .expect_err("observer cannot create goal forks")
    .into_response();
    assert_eq!(observer_create.status(), StatusCode::FORBIDDEN);
    let observer_complete = complete_goal_fork(
        State(state.clone()),
        observer_headers,
        Path((team.id.clone(), task.id.clone(), fork.id.clone())),
        Json(CompleteGoalForkRequest {
            result: json!({"summary":"unauthorized"}),
        }),
    )
    .await
    .expect_err("observer cannot complete goal forks")
    .into_response();
    assert_eq!(observer_complete.status(), StatusCode::FORBIDDEN);

    let Json(forks) = list_goal_forks(
        State(state.clone()),
        owner_headers.clone(),
        Path((team.id.clone(), task.id.clone())),
    )
    .await
    .expect("list goal forks");
    assert_eq!(forks, vec![fork.clone()]);
    let unauthorized = list_goal_forks(
        State(state.clone()),
        HeaderMap::new(),
        Path((team.id.clone(), task.id.clone())),
    )
    .await
    .expect_err("fork state requires authentication")
    .into_response();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let Json(completed) = complete_goal_fork(
        State(state.clone()),
        owner_headers.clone(),
        Path((team.id.clone(), task.id.clone(), fork.id.clone())),
        Json(CompleteGoalForkRequest {
            result: json!({"summary":"Source confirms the behavior"}),
        }),
    )
    .await
    .expect("complete goal fork");
    assert_eq!(
        completed.result,
        Some(json!({"summary":"Source confirms the behavior"}))
    );
    let notes = state
        .teams
        .list_task_notes(&task.id, 10)
        .await
        .expect("list task evidence");
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].text, "Source confirms the behavior");

    let duplicate = complete_goal_fork(
        State(state),
        owner_headers,
        Path((team.id, task.id, fork.id)),
        Json(CompleteGoalForkRequest {
            result: json!({"summary":"rewrite"}),
        }),
    )
    .await
    .expect_err("fork result is immutable")
    .into_response();
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);
}

#[cfg(feature = "object-store-s3")]
#[ignore = "Team multipart upload session routes are not implemented"]
#[tokio::test]
async fn team_upload_session_s3_multipart_route_fixture_publishes_metadata() {
    let Some(object_store) = s3_fixture_object_store_config_from_env() else {
        eprintln!("skipping s3 route fixture: AGENTHUB_OBJECT_STORE_S3_TEST_* env is not set");
        return;
    };
    let state = build_test_state_with_object_store(object_store).await;
    let token = create_auth_token(&state).await;
    let headers = auth_headers_for_token(&token);

    let Json(created) = create_team(
        State(state.clone()),
        headers,
        Json(CreateTeamRequest {
            name: "s3-multipart-upload-team".to_string(),
            description: None,
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create s3 multipart upload team");
    let app = super::router(state.clone());

    let bytes = vec![b'x'; 5 * 1024 * 1024];
    let sha256 = hex_encode(&Sha256::digest(&bytes));
    let prepared = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/{}/uploads/sessions", created.id),
            Some(&token),
            Some(json!({
                "file_name": "large-route-fixture.bin",
                "content_type": "application/octet-stream",
                "object_kind": "object",
                "expected_size_bytes": bytes.len(),
                "expected_sha256": sha256
            })),
        ))
        .await
        .expect("prepare s3 multipart upload session");
    let prepared_status = prepared.status();
    let session = decode_json_body(prepared).await;
    assert_eq!(
        prepared_status,
        StatusCode::OK,
        "unexpected prepare body: {session}"
    );
    let session_id = session["id"].as_str().expect("session id").to_string();

    let initiated = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/{}/uploads/sessions/{session_id}/multipart", created.id),
            Some(&token),
            None,
        ))
        .await
        .expect("initiate s3 multipart upload session");
    let initiated_status = initiated.status();
    let multipart = decode_json_body(initiated).await;
    assert_eq!(
        initiated_status,
        StatusCode::OK,
        "unexpected initiate body: {multipart}"
    );
    let upload_id = multipart["upload_id"]
        .as_str()
        .expect("multipart upload id")
        .to_string();

    let part_write = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!(
                "/{}/uploads/sessions/{session_id}/multipart/parts/1",
                created.id
            ),
            Some(&token),
            Some(json!({
                "upload_id": upload_id,
                "expires_in_seconds": 300
            })),
        ))
        .await
        .expect("prepare s3 multipart upload part");
    let part_write_status = part_write.status();
    let part = decode_json_body(part_write).await;
    assert_eq!(
        part_write_status,
        StatusCode::OK,
        "unexpected part write body: {part}"
    );
    assert_eq!(part["method"], Value::from("PUT"));

    let mut request = reqwest::Client::new().put(part["url"].as_str().expect("part url"));
    for header in part["headers"].as_array().expect("part headers") {
        let name = header["name"].as_str().expect("part header name");
        if name.eq_ignore_ascii_case("host") {
            continue;
        }
        request = request.header(name, header["value"].as_str().expect("part header value"));
    }
    let put_response = request
        .body(bytes.clone())
        .send()
        .await
        .expect("put presigned multipart part");
    let put_status = put_response.status();
    let etag = put_response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|value| value.to_str().ok())
        .expect("multipart part etag")
        .to_string();
    let put_body = put_response
        .text()
        .await
        .expect("read multipart put response body");
    assert!(
        put_status.is_success(),
        "unexpected multipart put status {put_status}: {put_body}"
    );

    let completed = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!(
                "/{}/uploads/sessions/{session_id}/multipart/complete",
                created.id
            ),
            Some(&token),
            Some(json!({
                "upload_id": upload_id,
                "parts": [{ "part_number": 1, "etag": etag }]
            })),
        ))
        .await
        .expect("complete s3 multipart upload session");
    let completed_status = completed.status();
    let upload = decode_json_body(completed).await;
    assert_eq!(
        completed_status,
        StatusCode::OK,
        "unexpected multipart complete body: {upload}"
    );
    assert_eq!(
        upload["owner_scope"],
        Value::from(format!("teams/{}", created.id))
    );
    assert_eq!(upload["publish_state"], Value::from("published"));
    assert_eq!(upload["size_bytes"], Value::from(bytes.len() as i64));
    let persisted = agenthub_db::object_uploads::get_object_upload(
        &state.db,
        upload["id"].as_str().expect("upload id"),
    )
    .await
    .expect("load persisted multipart upload");
    assert_eq!(persisted.publish_state, "published");
    assert_eq!(persisted.owner_scope, format!("teams/{}", created.id));

    let abort_prepared = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!("/{}/uploads/sessions", created.id),
            Some(&token),
            Some(json!({
                "file_name": "abort-route-fixture.bin",
                "content_type": "application/octet-stream",
                "object_kind": "object",
                "expected_size_bytes": bytes.len()
            })),
        ))
        .await
        .expect("prepare s3 multipart abort session");
    assert_eq!(abort_prepared.status(), StatusCode::OK);
    let abort_session = decode_json_body(abort_prepared).await;
    let abort_session_id = abort_session["id"]
        .as_str()
        .expect("abort session id")
        .to_string();
    let abort_initiated = app
        .clone()
        .oneshot(build_json_request(
            Method::POST,
            &format!(
                "/{}/uploads/sessions/{abort_session_id}/multipart",
                created.id
            ),
            Some(&token),
            None,
        ))
        .await
        .expect("initiate s3 multipart abort session");
    assert_eq!(abort_initiated.status(), StatusCode::OK);
    let abort_upload = decode_json_body(abort_initiated).await;
    let abort_upload_id = abort_upload["upload_id"]
        .as_str()
        .expect("abort upload id")
        .to_string();
    let aborted = app
        .oneshot(build_json_request(
            Method::POST,
            &format!(
                "/{}/uploads/sessions/{abort_session_id}/multipart/abort",
                created.id
            ),
            Some(&token),
            Some(json!({ "upload_id": abort_upload_id })),
        ))
        .await
        .expect("abort s3 multipart upload session");
    let aborted_status = aborted.status();
    let aborted_session = decode_json_body(aborted).await;
    assert_eq!(
        aborted_status,
        StatusCode::OK,
        "unexpected multipart abort body: {aborted_session}"
    );
    assert_eq!(aborted_session["status"], Value::from("canceled"));
}

#[cfg(feature = "object-store-s3")]
#[tokio::test]
async fn team_upload_s3_route_fixture_publishes_metadata() {
    let Some(object_store) = s3_fixture_object_store_config_from_env() else {
        eprintln!("skipping s3 route fixture: AGENTHUB_OBJECT_STORE_S3_TEST_* env is not set");
        return;
    };
    let state = build_test_state_with_object_store(object_store).await;
    let headers = auth_headers(&state).await;
    let Json(created) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: "s3-upload-team".to_string(),
            description: None,
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner","role":"coordinator"}]}),
        }),
    )
    .await
    .expect("create s3 upload team");

    let bytes = vec![b'x'; 5 * 1024 * 1024];
    let sha256 = hex_encode(&Sha256::digest(&bytes));
    let Json(upload) = upload_team_object(
        State(state.clone()),
        headers,
        Path(created.id.clone()),
        Json(TeamUploadRequest {
            file_name: "large-route-fixture.bin".to_string(),
            content_type: "application/octet-stream".to_string(),
            bytes_base64: STANDARD.encode(&bytes),
            expected_size_bytes: Some(bytes.len() as u64),
            expected_sha256: Some(sha256.clone()),
        }),
    )
    .await
    .expect("upload object through team route");

    assert_eq!(upload.owner_scope, format!("teams/{}", created.id));
    assert_eq!(upload.size_bytes, bytes.len() as i64);
    assert_eq!(upload.sha256, sha256);
    assert_eq!(upload.publish_state, "published");
    let persisted = agenthub_db::object_uploads::get_object_upload(&state.db, &upload.id)
        .await
        .expect("load persisted upload");
    assert_eq!(persisted, upload);
}

include!("tests_core.rs");
include!("tests_router.rs");

#[tokio::test]
async fn adopt_existing_agent_rejects_workspace_copy_destination_outside_safe_paths() {
    let state = build_test_state().await;
    let headers = auth_headers(&state).await;

    let source_workdir = std::env::temp_dir()
        .join(format!("agenthub-adopt-source-{}", Uuid::new_v4()))
        .to_string_lossy()
        .to_string();
    let source = state
        .agents
        .create_agent(crate::agent::AgentConfig {
            name: format!("adopt-source-{}", Uuid::new_v4()),
            workdir: source_workdir,
            command: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), "sleep 10".to_string()],
            target_node_id: None,
            worktree_mode: WorktreeMode::UseExisting,
            worktree_repo: None,
            worktree_ref: None,
            code_mode: true,
            codex_acp_default_mode: None,
            runtime_model: None,
            thinking_level: None,
            agent_loop_enabled: false,
            agent_loop_idle_seconds: None,
            agent_loop_prompt: None,
        })
        .await
        .expect("create standalone source agent");

    let Json(team) = create_team(
        State(state.clone()),
        headers.clone(),
        Json(CreateTeamRequest {
            name: format!("adopt-safe-path-team-{}", Uuid::new_v4()),
            description: None,
            spec: json!({
                "entrypoint": "coordinator",
                "members": [{"member_id": "coordinator", "role": "coordinator"}]
            }),
        }),
    )
    .await
    .expect("create team");

    // Configure a safe_paths allowlist that does NOT include the destination below, so the
    // workdir-validation gap this test guards against would otherwise let workspace-content
    // copy write agent files outside the operator-configured allowlist.
    let allowed = std::env::temp_dir()
        .join(format!("agenthub-adopt-allowed-{}", Uuid::new_v4()))
        .to_string_lossy()
        .to_string();
    sqlx::query("INSERT INTO safe_paths (path, created_at) VALUES (?1, ?2)")
        .bind(&allowed)
        .bind(Utc::now().timestamp())
        .execute(&state.db)
        .await
        .expect("insert allowed safe path");

    // Deliberately NOT under `std::env::temp_dir()`: `seed_default_team_member_agents` (called by
    // `build_test_state()`) unconditionally seeds a blanket "/tmp" safe_paths entry, and on Linux
    // `temp_dir()` *is* "/tmp" -- a temp-dir-based "outside" path would silently pass validation
    // there while still failing correctly on macOS, where `temp_dir()` differs from "/tmp".
    let outside_destination = format!("/agenthub-outside-safe-paths-allowlist-{}", Uuid::new_v4());

    let result = super::adopt_existing_agent_to_team(
        State(state.clone()),
        headers.clone(),
        Path(team.id.clone()),
        Json(super::AdoptExistingAgentRequest {
            source_agent_id: source.id.clone(),
            name: "adopted-agent".to_string(),
            spec: json!({
                "entrypoint": "coordinator",
                "members": [
                    {"member_id": "coordinator", "role": "coordinator"},
                    {"member_id": super::ADOPTED_MEMBER_ID_PLACEHOLDER, "role": "worker"}
                ]
            }),
            expected_updated_at: team.updated_at,
            workspace_copy_destination: Some(outside_destination),
            memory_seed: None,
        }),
    )
    .await;

    let err = result.expect_err("adoption must reject a destination outside safe_paths");
    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    let body: Value = serde_json::from_slice(&body).expect("decode response body");
    assert_eq!(
        body["error"],
        json!("workdir is outside the configured safe_paths allowlist")
    );

    // The source agent must still be unowned by any team -- the rejected adoption must not have
    // partially applied.
    let owning_teams = state
        .teams
        .list_teams_referencing_member(&source.id)
        .await
        .expect("list teams referencing source agent");
    assert!(owning_teams.is_empty());
}

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
            codex_acp_default_mode: None,
            runtime_model: None,
            thinking_level: None,
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
