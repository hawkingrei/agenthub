use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use super::codec::team_run_status_from_str;
use super::{TeamManager, TeamRunResumeError};
use crate::acp::{AcpActorSkillContext, DEFAULT_ACTOR_CHANNEL};
use crate::agent::{WorktreeMode, derive_team_runtime_workdir};
use crate::internal::client::InternalGrpcPeerClientConfig;
use crate::internal::tls::InternalGrpcSecurityMode;
use crate::team::{
    SendActorMessageInput, TeamActorMessageStatus, TeamActorMessageTransport, TeamDefinitionConfig,
    TeamRunStatus, TeamStepStatus, TeamTaskAssignmentUpdate, TeamTaskContextPatch,
    TeamTaskListQuery, TeamTaskStatus,
};
use agenthub_db::AgentEventDbRouter;
use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ACTOR_NODE_PEER_ID, ActorAckRequest, ActorIdentityKind, ActorInboxRequest,
    ActorMailboxService, ActorSendRequest, ActorServiceErrorCode,
};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::routing::any;
use axum::{Router, serve};
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use tokio::sync::Mutex;
use uuid::Uuid;

async fn setup_test_db() -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connect sqlite");

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
    .execute(&pool)
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
    .execute(&pool)
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
    .execute(&pool)
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
            FOREIGN KEY(run_id) REFERENCES team_runs(id)
        );
        "#,
    )
    .execute(&pool)
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
            assigned_member_id TEXT,
            context_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY(team_id) REFERENCES team_definitions(id)
        );
        "#,
    )
    .execute(&pool)
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
    .execute(&pool)
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
    .execute(&pool)
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
            idempotency_key TEXT,
            created_at INTEGER NOT NULL,
            FOREIGN KEY(conversation_id) REFERENCES team_conversations(id),
            FOREIGN KEY(task_id) REFERENCES team_tasks(id)
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("create team_conversation_messages");

    sqlx::query(
        r#"
        CREATE UNIQUE INDEX idx_team_conversation_messages_idempotency
        ON team_conversation_messages(conversation_id, from_actor_id, idempotency_key)
        WHERE idempotency_key IS NOT NULL;
        "#,
    )
    .execute(&pool)
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
    .execute(&pool)
    .await
    .expect("create team_actor_messages");

    sqlx::query(
        r#"
        CREATE TABLE team_channel_message_replicas (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            authority_message_id INTEGER NOT NULL,
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
    .execute(&pool)
    .await
    .expect("create team_channel_message_replicas");

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
    .execute(&pool)
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
    .execute(&pool)
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
    .execute(&pool)
    .await
    .expect("create team_context_flush_checkpoint");

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
            source TEXT NOT NULL DEFAULT 'manual',
            status TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("create agents");

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
    .execute(&pool)
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
    .execute(&pool)
    .await
    .expect("create agent_events");

    sqlx::query(
        r#"
        CREATE UNIQUE INDEX idx_team_actor_messages_idempotency
        ON team_actor_messages(run_id, from_actor_id, from_peer_id, idempotency_key)
        WHERE idempotency_key IS NOT NULL
        "#,
    )
    .execute(&pool)
    .await
    .expect("create team_actor_messages idempotency index");

    pool
}

async fn insert_team_conversation_message(
    db: &SqlitePool,
    conversation_id: &str,
    task_id: &str,
    from_actor_id: &str,
    payload_json: Value,
) -> i64 {
    let created_at = Utc::now().timestamp();
    sqlx::query(
        r#"
        INSERT INTO team_conversation_messages (
            conversation_id, task_id, from_actor_id, to_actor_id, route, payload_json, idempotency_key, created_at
        )
        VALUES (?1, ?2, ?3, NULL, 'broadcast', ?4, NULL, ?5)
        "#,
    )
    .bind(conversation_id)
    .bind(task_id)
    .bind(from_actor_id)
    .bind(payload_json.to_string())
    .bind(created_at)
    .execute(db)
    .await
    .expect("insert team conversation message")
    .last_insert_rowid()
}

#[derive(Debug, Clone)]
struct RelayHttpCapture {
    method: String,
    headers: HashMap<String, String>,
    body: serde_json::Value,
}

#[derive(Clone)]
struct RelayHttpState {
    captures: Arc<Mutex<Vec<RelayHttpCapture>>>,
    status: StatusCode,
}

async fn relay_http_handler(
    State(state): State<RelayHttpState>,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, &'static str) {
    let mut captured_headers = HashMap::new();
    for (name, value) in &headers {
        if let Ok(text) = value.to_str() {
            captured_headers.insert(name.as_str().to_string(), text.to_string());
        }
    }
    let captured_body = serde_json::from_slice::<serde_json::Value>(&body)
        .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&body).to_string()));
    state.captures.lock().await.push(RelayHttpCapture {
        method: method.as_str().to_string(),
        headers: captured_headers,
        body: captured_body,
    });
    (state.status, "ok")
}

async fn spawn_relay_http_server(
    status: StatusCode,
) -> (
    String,
    Arc<Mutex<Vec<RelayHttpCapture>>>,
    tokio::task::JoinHandle<()>,
) {
    let captures = Arc::new(Mutex::new(Vec::new()));
    let app = Router::new()
        .route("/relay", any(relay_http_handler))
        .route("/health", any(|| async { StatusCode::OK }))
        .with_state(RelayHttpState {
            captures: captures.clone(),
            status,
        });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind relay test server");
    let addr = listener.local_addr().expect("relay local addr");
    let handle = tokio::spawn(async move {
        let _ = serve(listener, app).await;
    });

    let endpoint = format!("http://{addr}/relay");
    let health_endpoint = format!("http://{addr}/health");
    let client = reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(std::time::Duration::from_millis(50))
        .timeout(std::time::Duration::from_millis(100))
        .build()
        .expect("build probe client");
    let mut ready = false;
    for _ in 0..50 {
        if let Ok(resp) = client.get(&health_endpoint).send().await
            && resp.status().is_success()
        {
            ready = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    if !ready {
        panic!("relay test server failed to become ready");
    }

    (endpoint, captures, handle)
}

#[test]
fn team_run_status_from_str_handles_submitted_and_unknown_values() {
    assert_eq!(
        team_run_status_from_str("submitted"),
        TeamRunStatus::Submitted
    );
    assert_eq!(
        team_run_status_from_str("unexpected"),
        TeamRunStatus::Submitted
    );
}

#[tokio::test]
async fn create_team_and_run_records_submission_event() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "review-team".to_string(),
            description: Some("team for review tasks".to_string()),
            spec: json!({"entrypoint":"triage","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");
    assert_eq!(team.name, "review-team");

    let run = manager
        .create_run(&team.id, None, json!({"prompt":"check plan"}))
        .await
        .expect("create run");
    assert_eq!(run.status, crate::team::TeamRunStatus::Submitted);
    assert_eq!(run.input["continuity"]["mode"], json!("inherit_recent"));

    let row = sqlx::query(
        "SELECT event_type, run_id, payload_json FROM team_run_events WHERE run_id = ?1 ORDER BY id ASC LIMIT 1",
    )
    .bind(&run.id)
    .fetch_one(&db)
    .await
    .expect("read run event");
    let event_type: String = row.get("event_type");
    let run_id: String = row.get("run_id");
    let payload_json: String = row.get("payload_json");
    let payload: serde_json::Value =
        serde_json::from_str(&payload_json).expect("decode run_submitted payload");
    assert_eq!(event_type, "run_submitted");
    assert_eq!(run_id, run.id);
    assert_eq!(payload["continuity_mode"], json!("inherit_recent"));
}

#[tokio::test]
async fn task_and_conversation_messages_are_persisted_with_redaction() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-team".to_string(),
            description: Some("team for task persistence".to_string()),
            spec: json!({"entrypoint":"leader_plan","members":[{"member_id":"leader"}]}),
        })
        .await
        .expect("create team");

    let (task, conversation) = manager
        .create_task(
            &team.id,
            "Investigate rollout plan",
            "user",
            json!({
                "source":"ui",
                "token":"should_not_persist",
                "nested":{"api_key":"xyz"}
            }),
            "group_chat",
            Some("kickoff"),
        )
        .await
        .expect("create task");
    assert_eq!(task.team_id, team.id);
    assert_eq!(task.status, TeamTaskStatus::Open);
    assert_eq!(task.assigned_member_id, None);
    assert_eq!(conversation.task_id, task.id);
    assert_eq!(task.context["token"], json!("[redacted]"));
    assert_eq!(task.context["nested"]["api_key"], json!("[redacted]"));

    let message = manager
        .append_task_conversation_message(
            &task.id,
            "leader",
            Some("worker-1"),
            "to_member",
            json!({
                "text":"draft changes",
                "authorization":"Bearer abc",
                "nested":{"secret":"top-secret"}
            }),
        )
        .await
        .expect("append message");
    assert_eq!(message.task_id, task.id);
    assert_eq!(message.payload["authorization"], json!("[redacted]"));
    assert_eq!(message.payload["nested"]["secret"], json!("[redacted]"));

    let listed = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: Some(team.id.clone()),
            limit: 20,
            ..TeamTaskListQuery::default()
        })
        .await
        .expect("list tasks");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, task.id);

    let messages = manager
        .list_task_conversation_messages(&task.id, 50, None)
        .await
        .expect("list conversation messages");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].message_id, message.message_id);
    assert_eq!(messages[0].route, "to_member");
}

#[tokio::test]
async fn append_task_conversation_message_emits_stream_event() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let mut events = manager.subscribe_conversation_events();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-event-team".to_string(),
            description: Some("team for conversation stream events".to_string()),
            spec: json!({"entrypoint":"leader_plan","members":[{"member_id":"leader"}]}),
        })
        .await
        .expect("create team");
    let (task, conversation) = manager
        .create_task(
            &team.id,
            "all",
            "user",
            json!({"bootstrap_kind":"shared_thread"}),
            "group_chat",
            Some("all"),
        )
        .await
        .expect("create task");

    let message = manager
        .append_task_conversation_message(
            &task.id,
            "user",
            None,
            "group_chat",
            json!({"type":"chat_message","text":"hello team"}),
        )
        .await
        .expect("append message");

    let event = tokio::time::timeout(std::time::Duration::from_millis(500), events.recv())
        .await
        .expect("receive stream event")
        .expect("stream event result");
    assert_eq!(event.team_id, team.id);
    assert_eq!(event.task_id, task.id);
    assert_eq!(event.conversation_id, conversation.id);
    assert_eq!(event.message_id, Some(message.message_id));
    assert_eq!(event.source, "conversation_message");
}

#[tokio::test]
async fn append_task_conversation_message_honors_idempotency_key() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let mut events = manager.subscribe_conversation_events();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-idempotency-team".to_string(),
            description: Some("team for task message idempotency".to_string()),
            spec: json!({"entrypoint":"leader_plan","members":[{"member_id":"leader"}]}),
        })
        .await
        .expect("create team");
    let (task, conversation) = manager
        .create_task(
            &team.id,
            "all",
            "user",
            json!({"bootstrap_kind":"shared_thread"}),
            "group_chat",
            Some("all"),
        )
        .await
        .expect("create task");

    let (first, first_created) = manager
        .append_task_conversation_message_with_created(
            &task.id,
            "user",
            None,
            "group_chat",
            json!({"type":"chat_message","text":"hello team"}),
            Some("task-msg-1"),
        )
        .await
        .expect("append first message");
    assert!(first_created);

    let (retry, retry_created) = manager
        .append_task_conversation_message_with_created(
            &task.id,
            "user",
            None,
            "group_chat",
            json!({"type":"chat_message","text":"hello team"}),
            Some("task-msg-1"),
        )
        .await
        .expect("append retry message");
    assert!(!retry_created);
    assert_eq!(first.message_id, retry.message_id);

    let event = tokio::time::timeout(std::time::Duration::from_millis(500), events.recv())
        .await
        .expect("receive first stream event")
        .expect("stream event result");
    assert_eq!(event.team_id, team.id);
    assert_eq!(event.task_id, task.id);
    assert_eq!(event.conversation_id, conversation.id);
    assert_eq!(event.message_id, Some(first.message_id));
    assert_eq!(event.source, "conversation_message");
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), events.recv())
            .await
            .is_err(),
        "retry should not emit a second stream event"
    );

    let err = manager
        .append_task_conversation_message_with_created(
            &task.id,
            "user",
            None,
            "group_chat",
            json!({"type":"chat_message","text":"changed payload"}),
            Some("task-msg-1"),
        )
        .await
        .expect_err("mismatched payload should conflict");
    assert!(
        TeamManager::is_task_message_idempotency_conflict(&err),
        "expected idempotency conflict, got: {err:?}"
    );

    let messages = manager
        .list_task_conversation_messages(&task.id, 50, None)
        .await
        .expect("list conversation messages");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].message_id, first.message_id);
}

#[tokio::test]
async fn append_task_conversation_message_propagates_non_idempotency_insert_failures() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-idempotency-insert-failure-team".to_string(),
            description: Some("team for task message insert failure".to_string()),
            spec: json!({"entrypoint":"leader_plan","members":[{"member_id":"leader"}]}),
        })
        .await
        .expect("create team");
    let (task, _) = manager
        .create_task(
            &team.id,
            "all",
            "user",
            json!({"bootstrap_kind":"shared_thread"}),
            "group_chat",
            Some("all"),
        )
        .await
        .expect("create task");

    sqlx::query(
        r#"
        CREATE TRIGGER fail_task_message_insert
        BEFORE INSERT ON team_conversation_messages
        WHEN NEW.idempotency_key IS NOT NULL
        BEGIN
            SELECT RAISE(FAIL, 'forced task message insert failure');
        END;
        "#,
    )
    .execute(&db)
    .await
    .expect("create failing trigger");

    let err = manager
        .append_task_conversation_message_with_created(
            &task.id,
            "user",
            None,
            "group_chat",
            json!({"type":"chat_message","text":"hello team"}),
            Some("task-msg-trigger-fail"),
        )
        .await
        .expect_err("trigger failure should propagate");
    assert!(
        err.to_string()
            .contains("forced task message insert failure"),
        "expected insert failure to propagate, got: {err:?}"
    );
    assert!(
        !TeamManager::is_task_message_idempotency_conflict(&err),
        "expected non-idempotency error, got: {err:?}"
    );
}

#[tokio::test]
async fn task_status_updates_are_persisted() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-status-team".to_string(),
            description: Some("team for task status updates".to_string()),
            spec: json!({"entrypoint":"leader_plan","members":[{"member_id":"leader"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Ship kanban",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("status"),
        )
        .await
        .expect("create task");
    assert_eq!(task.status, TeamTaskStatus::Open);

    let updated = manager
        .update_task_status(&task.id, TeamTaskStatus::InProgress)
        .await
        .expect("update task status");
    assert_eq!(updated.id, task.id);
    assert_eq!(updated.status, TeamTaskStatus::InProgress);

    let reloaded = manager
        .get_task(&task.id)
        .await
        .expect("reload updated task");
    assert_eq!(reloaded.status, TeamTaskStatus::InProgress);
    assert_eq!(reloaded.assigned_member_id, None);
}

#[tokio::test]
async fn task_assignment_updates_are_persisted() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-assignment-team".to_string(),
            description: Some("team for task assignment updates".to_string()),
            spec: json!({
                "entrypoint":"leader",
                "members":[
                    {"member_id":"leader","role":"leader"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Assign a worker",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("assignment"),
        )
        .await
        .expect("create task");
    assert_eq!(task.assigned_member_id, None);

    let assigned = manager
        .update_task(
            &task.id,
            None,
            TeamTaskAssignmentUpdate::Assigned("worker-1".to_string()),
        )
        .await
        .expect("assign task");
    assert_eq!(assigned.status, TeamTaskStatus::Open);
    assert_eq!(assigned.assigned_member_id.as_deref(), Some("worker-1"));

    let unassigned = manager
        .update_task(&task.id, None, TeamTaskAssignmentUpdate::Unassigned)
        .await
        .expect("unassign task");
    assert_eq!(unassigned.assigned_member_id, None);
}

#[tokio::test]
async fn task_partial_updates_preserve_unpatched_fields() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-partial-update-team".to_string(),
            description: Some("team for task patch semantics".to_string()),
            spec: json!({
                "entrypoint":"leader",
                "members":[
                    {"member_id":"leader","role":"leader"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Keep unrelated task fields intact",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("patch"),
        )
        .await
        .expect("create task");

    let assigned = manager
        .update_task(
            &task.id,
            None,
            TeamTaskAssignmentUpdate::Assigned("worker-1".to_string()),
        )
        .await
        .expect("assign task");
    assert_eq!(assigned.assigned_member_id.as_deref(), Some("worker-1"));
    assert_eq!(assigned.status, TeamTaskStatus::Open);

    let status_updated = manager
        .update_task_status(&task.id, TeamTaskStatus::InProgress)
        .await
        .expect("update task status");
    assert_eq!(status_updated.status, TeamTaskStatus::InProgress);
    assert_eq!(
        status_updated.assigned_member_id.as_deref(),
        Some("worker-1")
    );

    let unassigned = manager
        .update_task(&task.id, None, TeamTaskAssignmentUpdate::Unassigned)
        .await
        .expect("unassign task");
    assert_eq!(unassigned.status, TeamTaskStatus::InProgress);
    assert_eq!(unassigned.assigned_member_id, None);
}

#[tokio::test]
async fn task_context_patches_support_merge_and_replace() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-context-patch-team".to_string(),
            description: Some("team for task context patching".to_string()),
            spec: json!({"entrypoint":"leader","members":[{"member_id":"leader","role":"leader"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Patch task context",
            "leader",
            json!({"repo":"agenthub","nested":{"issue":128}}),
            "group_chat",
            Some("patch"),
        )
        .await
        .expect("create task");

    let merged = manager
        .update_task_with_context(
            &task.id,
            None,
            TeamTaskAssignmentUpdate::Unchanged,
            Some(TeamTaskContextPatch::Merge(json!({
                "nested":{"pr":227},
                "result":"done"
            }))),
        )
        .await
        .expect("merge task context");
    assert_eq!(merged.context["repo"], json!("agenthub"));
    assert_eq!(merged.context["nested"]["issue"], json!(128));
    assert_eq!(merged.context["nested"]["pr"], json!(227));
    assert_eq!(merged.context["result"], json!("done"));

    let replaced = manager
        .update_task_with_context(
            &task.id,
            Some(TeamTaskStatus::InReview),
            TeamTaskAssignmentUpdate::Unchanged,
            Some(TeamTaskContextPatch::Replace(json!({"owner":"leader"}))),
        )
        .await
        .expect("replace task context");
    assert_eq!(replaced.status, TeamTaskStatus::InReview);
    assert_eq!(replaced.context, json!({"owner":"leader"}));
}

#[tokio::test]
async fn create_task_rejects_invalid_reconcile_loop_execution_plan() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "invalid-execution-plan-team".to_string(),
            description: Some("team for invalid execution plan coverage".to_string()),
            spec: json!({
                "entrypoint":"leader",
                "members":[
                    {"member_id":"leader","role":"leader"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let err = manager
        .create_task(
            &team.id,
            "Invalid execution plan",
            "leader",
            json!({
                "execution_plan": {
                    "steps": [{
                        "step_key":"worker-implement",
                        "member_id":"worker-1",
                        "execution":{"mode":"reconcile_loop","max_rounds":0},
                        "acceptance":["tests pass"]
                    }]
                }
            }),
            "group_chat",
            Some("invalid"),
        )
        .await
        .expect_err("invalid reconcile loop plan should fail");
    assert!(
        err.to_string()
            .contains("reconcile_loop steps require a non-empty goal")
            || err
                .to_string()
                .contains("execution_plan.steps[].execution.max_rounds"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn update_task_context_rejects_execution_plan_with_unknown_member() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "unknown-member-execution-plan-team".to_string(),
            description: Some("team for execution plan member validation".to_string()),
            spec: json!({
                "entrypoint":"leader",
                "members":[
                    {"member_id":"leader","role":"leader"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Execution plan patch",
            "leader",
            json!({"repo":"agenthub"}),
            "group_chat",
            Some("patch"),
        )
        .await
        .expect("create task");

    let err = manager
        .update_task_with_context(
            &task.id,
            None,
            TeamTaskAssignmentUpdate::Unchanged,
            Some(TeamTaskContextPatch::Merge(json!({
                "execution_plan": {
                    "steps": [{
                        "step_key":"worker-implement",
                        "member_id":"missing-worker",
                        "execution":{"mode":"single_pass"}
                    }]
                }
            }))),
        )
        .await
        .expect_err("unknown member should fail validation");
    assert!(
        err.to_string()
            .contains("task context execution_plan.steps[].member_id must reference"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn list_tasks_with_query_filters_by_run_topic_and_owner() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-query-team".to_string(),
            description: Some("team for task query filtering".to_string()),
            spec: json!({
                "entrypoint":"leader",
                "members":[
                    {"member_id":"leader","role":"leader"},
                    {"member_id":"worker-1","role":"worker"},
                    {"member_id":"worker-2","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let (task_a, _) = manager
        .create_task(
            &team.id,
            "Leader task",
            "leader",
            json!({"source":"ui"}),
            "group_chat",
            Some("kanban"),
        )
        .await
        .expect("create first task");
    let (task_b, _) = manager
        .create_task(
            &team.id,
            "Worker task",
            "leader",
            json!({"source":"ui"}),
            "group_chat",
            Some("runtime"),
        )
        .await
        .expect("create second task");
    manager
        .update_task(
            &task_b.id,
            Some(TeamTaskStatus::InReview),
            TeamTaskAssignmentUpdate::Assigned("worker-2".to_string()),
        )
        .await
        .expect("assign task");
    let updated_task = manager
        .get_task(&task_b.id)
        .await
        .expect("reload updated task");
    assert_eq!(updated_task.status, TeamTaskStatus::InReview);
    assert_eq!(updated_task.assigned_member_id.as_deref(), Some("worker-2"));
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-task-query"),
            json!({"source":"query-scope"}),
        )
        .await
        .expect("create scope run");

    let scoped = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: None,
            run_id: Some(run.id.clone()),
            limit: 20,
            include_shared_thread: false,
            ..TeamTaskListQuery::default()
        })
        .await
        .expect("list run-scoped tasks");
    assert_eq!(scoped.len(), 2);

    let filtered_by_id = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: None,
            run_id: Some(run.id.clone()),
            limit: 20,
            task_id: Some(task_b.id.clone()),
            include_shared_thread: false,
            ..TeamTaskListQuery::default()
        })
        .await
        .expect("list task-id filtered tasks");
    assert_eq!(filtered_by_id.len(), 1);
    assert_eq!(filtered_by_id[0].id, task_b.id);

    let conversation = manager
        .get_task_conversation(&task_b.id)
        .await
        .expect("load task conversation");
    assert_eq!(conversation.topic.as_deref(), Some("runtime"));

    let filtered_by_status = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: None,
            run_id: Some(run.id.clone()),
            limit: 20,
            status: Some(TeamTaskStatus::InReview),
            task_id: Some(task_b.id.clone()),
            include_shared_thread: false,
            ..TeamTaskListQuery::default()
        })
        .await
        .expect("list status filtered tasks");
    assert_eq!(filtered_by_status.len(), 1);

    let filtered_by_owner = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: None,
            run_id: Some(run.id.clone()),
            limit: 20,
            task_id: Some(task_b.id.clone()),
            assigned_member_id: Some("worker-2".to_string()),
            include_shared_thread: false,
            ..TeamTaskListQuery::default()
        })
        .await
        .expect("list owner filtered tasks");
    assert_eq!(filtered_by_owner.len(), 1);

    let filtered_by_topic = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: None,
            run_id: Some(run.id.clone()),
            limit: 20,
            task_id: Some(task_b.id.clone()),
            topic: Some("runtime".to_string()),
            include_shared_thread: false,
            ..TeamTaskListQuery::default()
        })
        .await
        .expect("list topic filtered tasks");
    assert_eq!(filtered_by_topic.len(), 1);

    let listed = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: None,
            run_id: Some(run.id),
            limit: 20,
            status: Some(TeamTaskStatus::InReview),
            task_id: Some(task_b.id.clone()),
            assigned_member_id: Some("worker-2".to_string()),
            topic: Some("runtime".to_string()),
            include_shared_thread: false,
        })
        .await
        .expect("list filtered tasks");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, task_b.id);
    assert_ne!(listed[0].id, task_a.id);
}

#[tokio::test]
async fn create_run_marks_linked_task_in_progress() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "linked-task-run-team".to_string(),
            description: Some("team with linked task run".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Compile linked plan",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("linked-run"),
        )
        .await
        .expect("create task");

    let run = manager
        .create_run(
            &team.id,
            Some(&task.id),
            json!({"task_id": task.id, "prompt":"run linked task"}),
        )
        .await
        .expect("create linked run");
    assert_eq!(run.status, TeamRunStatus::Submitted);

    let reloaded = manager.get_task(&task.id).await.expect("reload task");
    assert_eq!(reloaded.status, TeamTaskStatus::InProgress);
}

#[tokio::test]
async fn create_run_materializes_input_step_template_into_run_steps() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "input-step-template-run-team".to_string(),
            description: Some("team with run input step template".to_string()),
            spec: json!({
                "entrypoint":"leader",
                "members":[
                    {"member_id":"leader","role":"leader"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let run = manager
        .create_run(
            &team.id,
            Some("ctx-step-template"),
            json!({
                "step_template": [
                    {
                        "step_key":"leader-plan",
                        "member_id":"leader",
                        "execution":{"mode":"single_pass"}
                    },
                    {
                        "step_key":"worker-implement",
                        "member_id":"worker-1",
                        "depends_on":["leader-plan"],
                        "goal":"finish the patch",
                        "acceptance":["tests pass"],
                        "execution":{"mode":"reconcile_loop","max_rounds":5}
                    }
                ]
            }),
        )
        .await
        .expect("create run");

    let steps = manager
        .list_steps(&run.id)
        .await
        .expect("list materialized steps");
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].step_key, "leader-plan");
    assert_eq!(steps[0].member_id, "leader");
    assert!(steps[0].depends_on.is_empty());
    assert_eq!(steps[0].input, None);
    assert_eq!(steps[1].step_key, "worker-implement");
    assert_eq!(steps[1].member_id, "worker-1");
    assert_eq!(steps[1].depends_on, vec!["leader-plan".to_string()]);
    assert_eq!(
        steps[1].input,
        Some(json!({
            "task_execution_step": {
                "goal":"finish the patch",
                "acceptance":["tests pass"],
                "execution":{"mode":"reconcile_loop","max_rounds":5},
                "round_state":{"current_round":0}
            }
        }))
    );
}

#[tokio::test]
async fn create_run_materializes_linked_task_execution_plan_when_input_has_no_step_template() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "linked-task-execution-plan-run-team".to_string(),
            description: Some("team with linked task execution plan".to_string()),
            spec: json!({
                "entrypoint":"leader",
                "members":[
                    {"member_id":"leader","role":"leader"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Execution-plan task",
            "leader",
            json!({
                "execution_plan": {
                    "steps": [
                        {
                            "step_key":"leader-plan",
                            "member_id":"leader",
                            "execution":{"mode":"single_pass"}
                        },
                        {
                            "step_key":"worker-implement",
                            "member_id":"worker-1",
                            "depends_on":["leader-plan"],
                            "goal":"finish implementation",
                            "acceptance":["tests pass","review notes addressed"],
                            "execution":{"mode":"reconcile_loop","max_rounds":4}
                        }
                    ]
                }
            }),
            "group_chat",
            Some("execution-plan"),
        )
        .await
        .expect("create task");

    let run = manager
        .create_run(
            &team.id,
            Some("ctx-linked-task-plan"),
            json!({"task_id": task.id, "prompt":"run linked task"}),
        )
        .await
        .expect("create run");

    let steps = manager.list_steps(&run.id).await.expect("list steps");
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].step_key, "leader-plan");
    assert_eq!(steps[1].step_key, "worker-implement");
    assert_eq!(steps[1].depends_on, vec!["leader-plan".to_string()]);
    assert_eq!(
        steps[1].input,
        Some(json!({
            "task_execution_step": {
                "goal":"finish implementation",
                "acceptance":["tests pass","review notes addressed"],
                "execution":{"mode":"reconcile_loop","max_rounds":4},
                "round_state":{"current_round":0}
            }
        }))
    );
}

#[tokio::test]
async fn create_run_rejects_invalid_input_step_template_member_scope() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "invalid-step-template-run-team".to_string(),
            description: Some("team with invalid run input step template".to_string()),
            spec: json!({
                "entrypoint":"leader",
                "members":[
                    {"member_id":"leader","role":"leader"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let err = manager
        .create_run(
            &team.id,
            Some("ctx-invalid-step-template"),
            json!({
                "step_template": [{
                    "step_key":"worker-implement",
                    "member_id":"worker-2",
                    "goal":"finish the patch",
                    "acceptance":["tests pass"],
                    "execution":{"mode":"reconcile_loop","max_rounds":5}
                }]
            }),
        )
        .await
        .expect_err("invalid step template should fail");
    assert!(
        err.to_string()
            .contains("run input step_template[].member_id must reference"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn create_run_hides_cross_team_linked_task_lookup_details() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team_a = manager
        .create_team(TeamDefinitionConfig {
            name: "run-team-a".to_string(),
            description: Some("requesting team".to_string()),
            spec: json!({"entrypoint":"leader","members":[{"member_id":"leader","role":"leader"}]}),
        })
        .await
        .expect("create team a");
    let team_b = manager
        .create_team(TeamDefinitionConfig {
            name: "run-team-b".to_string(),
            description: Some("foreign team".to_string()),
            spec: json!({"entrypoint":"leader","members":[{"member_id":"leader","role":"leader"}]}),
        })
        .await
        .expect("create team b");

    let (foreign_task, _) = manager
        .create_task(
            &team_b.id,
            "Foreign task",
            "leader",
            json!({"source":"foreign"}),
            "group_chat",
            Some("foreign-task"),
        )
        .await
        .expect("create foreign task");

    let wrong_team_err = manager
        .create_run(
            &team_a.id,
            Some("ctx-cross-team"),
            json!({"task_id": foreign_task.id, "prompt":"run foreign task"}),
        )
        .await
        .expect_err("cross-team task should fail");
    let missing_task_err = manager
        .create_run(
            &team_a.id,
            Some("ctx-missing-task"),
            json!({"task_id": "missing-task", "prompt":"run missing task"}),
        )
        .await
        .expect_err("missing task should fail");

    assert_eq!(
        wrong_team_err.to_string(),
        "linked task does not belong to the requested team"
    );
    assert_eq!(wrong_team_err.to_string(), missing_task_err.to_string());
}

#[tokio::test]
async fn list_tasks_with_query_hides_shared_thread_bootstrap_kind_case_insensitively() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-shared-thread-casefold".to_string(),
            description: Some(
                "verify shared thread filtering remains case-insensitive".to_string(),
            ),
            spec: json!({
                "entrypoint":"leader",
                "members":[
                    {"member_id":"leader","role":"leader"},
                    {"member_id":"worker","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let (visible_task, _) = manager
        .create_task(
            &team.id,
            "Normal task",
            "leader",
            json!({"topic":"visible"}),
            "group_chat",
            Some("visible"),
        )
        .await
        .expect("create visible task");
    manager
        .create_task(
            &team.id,
            "Shared thread",
            "leader",
            json!({"bootstrap_kind":"Shared_Thread"}),
            "group_chat",
            Some("shared"),
        )
        .await
        .expect("create shared thread task");

    let listed = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: Some(team.id.clone()),
            limit: 20,
            include_shared_thread: false,
            ..TeamTaskListQuery::default()
        })
        .await
        .expect("list visible tasks");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, visible_task.id);
}

#[tokio::test]
async fn create_team_channel_creates_bootstrap_conversation_and_hides_it_from_task_list() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "team-channel-create".to_string(),
            description: Some("verify channel bootstrap records".to_string()),
            spec: json!({
                "entrypoint":"leader",
                "members":[
                    {"member_id":"leader","role":"leader"},
                    {"member_id":"worker","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let channel = manager
        .create_channel(&team.id, "review", Some("Review queue"), "leader")
        .await
        .expect("create review channel");
    assert_eq!(channel.team_id, team.id);
    assert_eq!(channel.channel_id, "review");
    assert_ne!(channel.conversation_id, "review");
    assert_eq!(channel.description.as_deref(), Some("Review queue"));
    assert_eq!(channel.created_by_actor_id, "leader");

    let conversation = sqlx::query(
        r#"
        SELECT c.id, c.task_id, t.context_json
        FROM team_conversations c
        INNER JOIN team_tasks t ON t.id = c.task_id
        WHERE c.team_id = ?1 AND c.id = ?2
        LIMIT 1
        "#,
    )
    .bind(&team.id)
    .bind(&channel.conversation_id)
    .fetch_one(&db)
    .await
    .expect("fetch review conversation");
    assert_eq!(conversation.get::<String, _>("id"), channel.conversation_id);
    assert_eq!(conversation.get::<String, _>("task_id"), channel.task_id);
    let context_json: Value = serde_json::from_str(&conversation.get::<String, _>("context_json"))
        .expect("parse context");
    assert_eq!(context_json["bootstrap_kind"], "team_channel");
    assert_eq!(context_json["bootstrap_source"], "leader_created");
    assert_eq!(context_json["channel_id"], "review");
    assert_eq!(context_json["description"], "Review queue");

    let listed = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: Some(team.id.clone()),
            limit: 20,
            include_shared_thread: false,
            ..TeamTaskListQuery::default()
        })
        .await
        .expect("list visible tasks");
    assert!(
        listed.is_empty(),
        "channel bootstrap tasks should stay hidden"
    );
}

#[tokio::test]
async fn create_team_channel_allows_same_channel_id_in_different_teams() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let team_a = manager
        .create_team(TeamDefinitionConfig {
            name: "team-channel-a".to_string(),
            description: Some("team a".to_string()),
            spec: json!({
                "entrypoint":"leader",
                "members":[{"member_id":"leader","role":"leader"}]
            }),
        })
        .await
        .expect("create team a");
    let team_b = manager
        .create_team(TeamDefinitionConfig {
            name: "team-channel-b".to_string(),
            description: Some("team b".to_string()),
            spec: json!({
                "entrypoint":"leader",
                "members":[{"member_id":"leader","role":"leader"}]
            }),
        })
        .await
        .expect("create team b");

    let review_a = manager
        .create_channel(&team_a.id, "review", Some("Review lane"), "leader")
        .await
        .expect("create review channel for team a");
    let review_b = manager
        .create_channel(&team_b.id, "review", Some("Review lane"), "leader")
        .await
        .expect("create review channel for team b");

    assert_ne!(review_a.conversation_id, review_b.conversation_id);

    let conversation_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM team_tasks t
        INNER JOIN team_conversations c ON c.task_id = t.id
        WHERE lower(trim(COALESCE(json_extract(t.context_json, '$.bootstrap_kind'), ''))) = 'team_channel'
          AND lower(trim(COALESCE(json_extract(t.context_json, '$.channel_id'), ''))) = 'review'
        "#,
    )
    .fetch_one(&db)
    .await
    .expect("count review channel bootstraps");
    assert_eq!(conversation_count, 2);
}

#[tokio::test]
async fn create_team_channel_canonicalizes_case_and_rejects_same_team_duplicates() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "team-channel-case".to_string(),
            description: Some("verify channel canonicalization".to_string()),
            spec: json!({
                "entrypoint":"leader",
                "members":[{"member_id":"leader","role":"leader"}]
            }),
        })
        .await
        .expect("create team");

    let channel = manager
        .create_channel(&team.id, " Review ", Some("Review lane"), "leader")
        .await
        .expect("create review channel");
    assert_eq!(channel.channel_id, "review");

    let duplicate = manager
        .create_channel(&team.id, "REVIEW", Some("Duplicate review lane"), "leader")
        .await
        .expect_err("duplicate review channel should fail");
    assert!(
        duplicate
            .to_string()
            .contains("channel 'review' already exists")
    );
}

#[tokio::test]
async fn delete_team_channel_cleans_bootstrap_rows_and_rejects_all() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "team-channel-delete".to_string(),
            description: Some("verify channel deletion cleanup".to_string()),
            spec: json!({
                "entrypoint":"leader",
                "members":[
                    {"member_id":"leader","role":"leader"},
                    {"member_id":"worker","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let cannot_delete_all = manager
        .delete_channel(&team.id, "all")
        .await
        .expect_err("shared channel should be reserved");
    assert!(
        cannot_delete_all
            .to_string()
            .contains("channel_id 'all' cannot be deleted")
    );

    let channel = manager
        .create_channel(&team.id, "research", Some("Research lane"), "leader")
        .await
        .expect("create research channel");

    let root_message_id = insert_team_conversation_message(
        &db,
        &channel.conversation_id,
        &channel.task_id,
        "leader",
        json!({"text":"Investigate issue"}),
    )
    .await;
    sqlx::query(
        r#"
        INSERT INTO team_channel_message_replicas (
            authority_message_id, run_id, team_id, conversation_id, task_id, channel_id, from_actor_id, source_node_id, payload_json, stored_at
        )
        VALUES (?1, 'run-1', ?2, ?3, ?4, ?5, 'leader', 'main', '{"text":"Investigate issue"}', ?6)
        "#,
    )
    .bind(root_message_id)
    .bind(&team.id)
    .bind(&channel.conversation_id)
    .bind(&channel.task_id)
    .bind(&channel.channel_id)
    .bind(Utc::now().timestamp())
    .execute(&db)
    .await
    .expect("insert channel replica");

    let deleted = manager
        .delete_channel(&team.id, "research")
        .await
        .expect("delete research channel");
    assert_eq!(deleted.channel_id, "research");
    assert_eq!(deleted.task_id, channel.task_id);

    let remaining_conversations =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM team_conversations WHERE id = ?1")
            .bind(&channel.conversation_id)
            .fetch_one(&db)
            .await
            .expect("count conversations");
    let remaining_tasks =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM team_tasks WHERE id = ?1")
            .bind(&channel.task_id)
            .fetch_one(&db)
            .await
            .expect("count tasks");
    let remaining_messages = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM team_conversation_messages WHERE conversation_id = ?1",
    )
    .bind(&channel.conversation_id)
    .fetch_one(&db)
    .await
    .expect("count messages");
    let remaining_replicas = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM team_channel_message_replicas WHERE conversation_id = ?1",
    )
    .bind(&channel.conversation_id)
    .fetch_one(&db)
    .await
    .expect("count replicas");

    assert_eq!(remaining_conversations, 0);
    assert_eq!(remaining_tasks, 0);
    assert_eq!(remaining_messages, 0);
    assert_eq!(remaining_replicas, 0);
}

#[tokio::test]
async fn delete_team_channel_returns_canonical_channel_id() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "team-delete-case".to_string(),
            description: Some("verify delete canonicalization".to_string()),
            spec: json!({
                "entrypoint":"leader",
                "members":[{"member_id":"leader","role":"leader"}]
            }),
        })
        .await
        .expect("create team");

    manager
        .create_channel(&team.id, "Review", Some("Review lane"), "leader")
        .await
        .expect("create review channel");
    let deleted = manager
        .delete_channel(&team.id, " REVIEW ")
        .await
        .expect("delete review channel");

    assert_eq!(deleted.channel_id, "review");
}

#[tokio::test]
async fn delete_team_channel_does_not_touch_other_team_same_channel_id() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let team_a = manager
        .create_team(TeamDefinitionConfig {
            name: "team-delete-a".to_string(),
            description: Some("team a".to_string()),
            spec: json!({
                "entrypoint":"leader",
                "members":[{"member_id":"leader","role":"leader"}]
            }),
        })
        .await
        .expect("create team a");
    let team_b = manager
        .create_team(TeamDefinitionConfig {
            name: "team-delete-b".to_string(),
            description: Some("team b".to_string()),
            spec: json!({
                "entrypoint":"leader",
                "members":[{"member_id":"leader","role":"leader"}]
            }),
        })
        .await
        .expect("create team b");

    let review_a = manager
        .create_channel(&team_a.id, "review", Some("Review lane"), "leader")
        .await
        .expect("create review channel for team a");
    let review_b = manager
        .create_channel(&team_b.id, "review", Some("Review lane"), "leader")
        .await
        .expect("create review channel for team b");

    manager
        .delete_channel(&team_a.id, "review")
        .await
        .expect("delete review channel for team a");

    let surviving_conversation =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM team_conversations WHERE id = ?1")
            .bind(&review_b.conversation_id)
            .fetch_one(&db)
            .await
            .expect("count surviving conversation");
    let surviving_task =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM team_tasks WHERE id = ?1")
            .bind(&review_b.task_id)
            .fetch_one(&db)
            .await
            .expect("count surviving task");
    let deleted_conversation =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM team_conversations WHERE id = ?1")
            .bind(&review_a.conversation_id)
            .fetch_one(&db)
            .await
            .expect("count deleted conversation");

    assert_eq!(surviving_conversation, 1);
    assert_eq!(surviving_task, 1);
    assert_eq!(deleted_conversation, 0);
}

#[tokio::test]
async fn open_team_thread_supports_shared_and_custom_channels() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "team-open-thread".to_string(),
            description: Some("verify open thread routes".to_string()),
            spec: json!({
                "entrypoint":"leader",
                "members":[
                    {"member_id":"leader","role":"leader"},
                    {"member_id":"worker","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let (shared_task, shared_conversation) = manager
        .create_task(
            &team.id,
            "all",
            "leader",
            json!({"bootstrap_kind":"shared_thread"}),
            "group_chat",
            Some("all"),
        )
        .await
        .expect("create shared thread target");
    let shared_conversation_id = shared_conversation.id;
    let shared_task_id = shared_task.id;
    let shared_root_message_id = insert_team_conversation_message(
        &db,
        &shared_conversation_id,
        &shared_task_id,
        "leader",
        json!({"text":"Shared update"}),
    )
    .await;

    let shared_thread = manager
        .open_thread(&team.id, "all", shared_root_message_id)
        .await
        .expect("open shared thread");
    assert_eq!(shared_thread.channel_id, "all");
    assert_eq!(shared_thread.conversation_id, shared_conversation_id);
    assert_eq!(shared_thread.task_id, shared_task_id);
    assert_eq!(shared_thread.root_message_id, shared_root_message_id);
    assert_eq!(shared_thread.thread_id, shared_root_message_id.to_string());

    let channel = manager
        .create_channel(&team.id, "review", Some("Review lane"), "leader")
        .await
        .expect("create review channel");
    let review_root_message_id = insert_team_conversation_message(
        &db,
        &channel.conversation_id,
        &channel.task_id,
        "leader",
        json!({"text":"Please review"}),
    )
    .await;

    let review_thread = manager
        .open_thread(&team.id, "ReViEw", review_root_message_id)
        .await
        .expect("open review thread");
    assert_eq!(review_thread.channel_id, "review");
    assert_eq!(review_thread.conversation_id, channel.conversation_id);
    assert_eq!(review_thread.task_id, channel.task_id);
    assert_eq!(review_thread.root_message_id, review_root_message_id);
    assert_eq!(review_thread.thread_id, review_root_message_id.to_string());
}

#[tokio::test]
async fn list_tasks_with_query_keeps_tasks_without_conversation_rows() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-query-left-join".to_string(),
            description: Some("list tasks should not require conversation rows".to_string()),
            spec: json!({
                "entrypoint":"leader",
                "members":[
                    {"member_id":"leader","role":"leader"},
                    {"member_id":"worker","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let (orphan_task, _) = manager
        .create_task(
            &team.id,
            "Legacy orphan task",
            "leader",
            json!({"source":"legacy"}),
            "group_chat",
            Some("legacy"),
        )
        .await
        .expect("create orphan task");
    sqlx::query("DELETE FROM team_conversations WHERE task_id = ?1")
        .bind(&orphan_task.id)
        .execute(&db)
        .await
        .expect("delete orphan task conversation");

    let (topic_task, _) = manager
        .create_task(
            &team.id,
            "Topic task",
            "leader",
            json!({"source":"ui"}),
            "group_chat",
            Some("topic-a"),
        )
        .await
        .expect("create topic task");

    let listed = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: Some(team.id.clone()),
            limit: 20,
            include_shared_thread: false,
            ..TeamTaskListQuery::default()
        })
        .await
        .expect("list tasks with orphan row");
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().any(|task| task.id == orphan_task.id));
    assert!(listed.iter().any(|task| task.id == topic_task.id));

    let topic_filtered = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: Some(team.id),
            limit: 20,
            topic: Some("topic-a".to_string()),
            include_shared_thread: false,
            ..TeamTaskListQuery::default()
        })
        .await
        .expect("list topic filtered tasks");
    assert_eq!(topic_filtered.len(), 1);
    assert_eq!(topic_filtered[0].id, topic_task.id);
}

#[tokio::test]
async fn linked_run_completion_marks_task_in_review() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "linked-task-complete-team".to_string(),
            description: Some("team with linked run completion".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Ship linked completion",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("complete"),
        )
        .await
        .expect("create task");

    let run = manager
        .create_run(
            &team.id,
            Some(&task.id),
            json!({"task_id": task.id, "prompt":"finish linked task"}),
        )
        .await
        .expect("create linked run");
    let step = manager
        .submit_step(
            &run.id,
            "planner_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"complete linked task"})),
        )
        .await
        .expect("submit step");
    let _ = manager
        .start_step(&step.id, Some("linked-session-complete"))
        .await
        .expect("start step");
    let _ = manager
        .complete_step(&step.id, Some(json!({"result":"done"})))
        .await
        .expect("complete step");

    let reloaded = manager.get_task(&task.id).await.expect("reload task");
    assert_eq!(reloaded.status, TeamTaskStatus::InReview);
}

#[tokio::test]
async fn linked_run_sync_preserves_waiting_tasks() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "linked-task-waiting-team".to_string(),
            description: Some("team with sticky waiting tasks".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Wait for review",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            None,
        )
        .await
        .expect("create task");
    let task = manager
        .update_task_status(&task.id, TeamTaskStatus::Waiting)
        .await
        .expect("move task to waiting");
    assert_eq!(task.status, TeamTaskStatus::Waiting);

    let run = manager
        .create_run(
            &team.id,
            Some(&task.id),
            json!({"task_id": task.id, "prompt":"check for review updates"}),
        )
        .await
        .expect("create linked run");
    let after_create = manager
        .get_task(&task.id)
        .await
        .expect("reload after create");
    assert_eq!(after_create.status, TeamTaskStatus::Waiting);

    let step = manager
        .submit_step(
            &run.id,
            "planner_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"check waiting dependency"})),
        )
        .await
        .expect("submit step");
    let _ = manager
        .start_step(&step.id, Some("linked-session-waiting"))
        .await
        .expect("start step");
    let _ = manager
        .complete_step(&step.id, Some(json!({"result":"still waiting"})))
        .await
        .expect("complete step");

    let reloaded = manager.get_task(&task.id).await.expect("reload task");
    assert_eq!(reloaded.status, TeamTaskStatus::Waiting);
}

#[tokio::test]
async fn linked_run_input_required_and_resume_sync_task_waiting_transitions() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "linked-task-waiting-transition-team".to_string(),
            description: Some("team with linked waiting/resume transitions".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Wait for approval and resume",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("waiting-transition"),
        )
        .await
        .expect("create task");

    let run = manager
        .create_run(
            &team.id,
            Some(&task.id),
            json!({"task_id": task.id, "prompt":"pause for approval then resume"}),
        )
        .await
        .expect("create linked run");
    let step = manager
        .submit_step(
            &run.id,
            "planner_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"wait for approval"})),
        )
        .await
        .expect("submit step");
    let _ = manager
        .start_step(&step.id, Some("linked-session-waiting-transition"))
        .await
        .expect("start step");

    let after_start = manager
        .get_task(&task.id)
        .await
        .expect("reload after start");
    assert_eq!(after_start.status, TeamTaskStatus::InProgress);

    let _ = manager
        .set_step_input_required(
            &step.id,
            Some("approval is required"),
            Some(json!({"question":"approve?"})),
        )
        .await
        .expect("mark input required");
    let after_input_required = manager
        .get_task(&task.id)
        .await
        .expect("reload after input required");
    assert_eq!(after_input_required.status, TeamTaskStatus::Waiting);

    let _ = manager
        .resume_step(&step.id, Some(json!({"answer":"approved"})))
        .await
        .expect("resume step");
    let after_resume = manager
        .get_task(&task.id)
        .await
        .expect("reload after resume");
    assert_eq!(after_resume.status, TeamTaskStatus::InProgress);

    let _ = manager
        .complete_step(&step.id, Some(json!({"result":"done"})))
        .await
        .expect("complete step");
    let after_complete = manager
        .get_task(&task.id)
        .await
        .expect("reload after complete");
    assert_eq!(after_complete.status, TeamTaskStatus::InReview);
}

#[tokio::test]
async fn cancel_run_preserves_waiting_task() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "linked-task-waiting-cancel-team".to_string(),
            description: Some("team with sticky waiting cancellation".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Wait for approval",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            None,
        )
        .await
        .expect("create task");
    let task = manager
        .update_task_status(&task.id, TeamTaskStatus::Waiting)
        .await
        .expect("move task to waiting");
    assert_eq!(task.status, TeamTaskStatus::Waiting);

    let run = manager
        .create_run(
            &team.id,
            Some(&task.id),
            json!({"task_id": task.id, "prompt":"check waiting dependency"}),
        )
        .await
        .expect("create linked run");
    let _ = manager.cancel_run(&run.id).await.expect("cancel run");

    let reloaded = manager.get_task(&task.id).await.expect("reload task");
    assert_eq!(reloaded.status, TeamTaskStatus::Waiting);
}

#[tokio::test]
async fn linked_run_failure_keeps_task_in_progress() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "linked-task-fail-team".to_string(),
            description: Some("team with linked run failure".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Handle linked failure",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("failure"),
        )
        .await
        .expect("create task");

    let run = manager
        .create_run(
            &team.id,
            Some(&task.id),
            json!({"task_id": task.id, "prompt":"fail linked task"}),
        )
        .await
        .expect("create linked run");
    let step = manager
        .submit_step(
            &run.id,
            "planner_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"fail linked task"})),
        )
        .await
        .expect("submit step");
    let _ = manager
        .start_step(&step.id, Some("linked-session-fail"))
        .await
        .expect("start step");
    let _ = manager
        .fail_step(&step.id, "linked run failed")
        .await
        .expect("fail step");

    let reloaded = manager.get_task(&task.id).await.expect("reload task");
    assert_eq!(reloaded.status, TeamTaskStatus::InProgress);
}

#[tokio::test]
async fn cancel_run_marks_linked_task_canceled() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "linked-task-cancel-team".to_string(),
            description: Some("team with linked run cancellation".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Cancel linked run",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("cancel"),
        )
        .await
        .expect("create task");

    let run = manager
        .create_run(
            &team.id,
            Some(&task.id),
            json!({"task_id": task.id, "prompt":"cancel linked task"}),
        )
        .await
        .expect("create linked run");
    let _ = manager.cancel_run(&run.id).await.expect("cancel run");

    let reloaded = manager.get_task(&task.id).await.expect("reload task");
    assert_eq!(reloaded.status, TeamTaskStatus::Canceled);
}

#[tokio::test]
async fn startup_cancellation_reopens_linked_task() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "linked-task-startup-cancel-team".to_string(),
            description: Some("team with startup run cancellation".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Resume after restart",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("startup-cancel"),
        )
        .await
        .expect("create task");

    let run = manager
        .create_run(
            &team.id,
            Some(&task.id),
            json!({"task_id": task.id, "prompt":"restart-sensitive task"}),
        )
        .await
        .expect("create linked run");
    assert_eq!(
        manager
            .get_task(&task.id)
            .await
            .expect("reload in-progress task")
            .status,
        TeamTaskStatus::InProgress
    );

    let canceled_count = manager
        .cancel_active_runs_on_startup()
        .await
        .expect("cancel active runs on startup");
    assert_eq!(canceled_count, 1);
    assert_eq!(
        manager
            .get_run(&run.id)
            .await
            .expect("reload canceled run")
            .status,
        TeamRunStatus::Canceled
    );
    assert_eq!(
        manager
            .get_task(&task.id)
            .await
            .expect("reload reopened task")
            .status,
        TeamTaskStatus::Open
    );
}

#[tokio::test]
async fn update_team_spec_if_unchanged_detects_conflict_and_updates_on_match() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "update-team-spec".to_string(),
            description: Some("optimistic lock".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let conflicted = manager
        .update_team_spec_if_unchanged(
            &team.id,
            team.updated_at - 1,
            json!({"entrypoint":"stale","members":[{"member_id":"planner"}]}),
        )
        .await
        .expect("stale update should not fail");
    assert!(conflicted.is_none());

    let updated = manager
        .update_team_spec_if_unchanged(
            &team.id,
            team.updated_at,
            json!({"entrypoint":"updated","members":[{"member_id":"planner"}]}),
        )
        .await
        .expect("matching update")
        .expect("team should be updated");
    assert_eq!(updated.spec["entrypoint"], json!("updated"));
}

#[tokio::test]
async fn update_team_spec_if_unchanged_returns_not_found_for_missing_team() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db);

    let err = manager
        .update_team_spec_if_unchanged(
            "missing-team",
            0,
            json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        )
        .await
        .expect_err("missing team should fail");

    assert!(matches!(
        err.downcast_ref::<sqlx::Error>(),
        Some(sqlx::Error::RowNotFound)
    ));
}

#[tokio::test]
async fn cancel_run_updates_status_and_emits_event() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "cancel-team".to_string(),
            description: None,
            spec: json!({"entrypoint":"main","members":[]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-1"), json!({"payload":1}))
        .await
        .expect("create run");

    let canceled = manager.cancel_run(&run.id).await.expect("cancel run");
    assert_eq!(canceled.status, crate::team::TeamRunStatus::Canceled);
    assert!(canceled.ended_at.is_some());

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event_type, "run_submitted");
    assert_eq!(events[1].event_type, "run_canceled");
}

#[tokio::test]
async fn cancel_run_only_cancels_active_steps() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "cancel-active-step-team".to_string(),
            description: None,
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-cancel-steps"), json!({"payload":1}))
        .await
        .expect("create run");

    let completed_step = manager
        .submit_step(
            &run.id,
            "already_done",
            "planner",
            Vec::new(),
            Some(json!({"goal":"done"})),
        )
        .await
        .expect("submit completed step");
    let active_step = manager
        .submit_step(
            &run.id,
            "still_running",
            "planner",
            Vec::new(),
            Some(json!({"goal":"running"})),
        )
        .await
        .expect("submit active step");
    let _ = manager
        .start_step(&completed_step.id, Some("remote-completed"))
        .await
        .expect("start completed step");
    let _ = manager
        .start_step(&active_step.id, Some("remote-active"))
        .await
        .expect("start active step");
    let _ = manager
        .complete_step(&completed_step.id, Some(json!({"result":"ok"})))
        .await
        .expect("complete step");

    let canceled_run = manager.cancel_run(&run.id).await.expect("cancel run");
    assert_eq!(canceled_run.status, TeamRunStatus::Canceled);

    let completed_after_cancel = manager
        .get_step(&completed_step.id)
        .await
        .expect("get completed step");
    assert_eq!(completed_after_cancel.status, TeamStepStatus::Completed);

    let active_after_cancel = manager
        .get_step(&active_step.id)
        .await
        .expect("get active step");
    assert_eq!(active_after_cancel.status, TeamStepStatus::Canceled);
    assert!(active_after_cancel.ended_at.is_some());

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let canceled_step_ids: Vec<String> = events
        .iter()
        .filter(|event| event.event_type == "step_canceled")
        .filter_map(|event| event.step_id.clone())
        .collect();
    assert_eq!(canceled_step_ids, vec![active_step.id]);
}

#[tokio::test]
async fn step_lifecycle_transitions_persist_and_emit_events() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "step-team".to_string(),
            description: Some("team with step lifecycle".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-step"), json!({"payload":"start"}))
        .await
        .expect("create run");

    let step = manager
        .submit_step(
            &run.id,
            "plan_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"draft plan"})),
        )
        .await
        .expect("submit step");
    assert_eq!(step.status, TeamStepStatus::Submitted);

    let working = manager
        .start_step(&step.id, Some("remote-task-1"))
        .await
        .expect("start step");
    assert_eq!(working.status, TeamStepStatus::Working);
    assert_eq!(working.runtime_handle_id.as_deref(), Some("remote-task-1"));
    assert!(working.started_at.is_some());

    let run_after_start = manager.get_run(&run.id).await.expect("get run");
    assert_eq!(run_after_start.status, TeamRunStatus::Working);
    assert!(run_after_start.started_at.is_some());

    let completed = manager
        .complete_step(&step.id, Some(json!({"result":"ok"})))
        .await
        .expect("complete step");
    assert_eq!(completed.status, TeamStepStatus::Completed);
    assert_eq!(completed.output, Some(json!({"result":"ok"})));
    assert!(completed.ended_at.is_some());

    let continuity = manager
        .get_member_continuity_state(&team.id, "planner")
        .await
        .expect("get continuity state")
        .expect("continuity state should exist");
    assert_eq!(continuity.team_id, team.id);
    assert_eq!(continuity.member_id, "planner");
    assert_eq!(continuity.source_run_id, run.id);
    assert_eq!(
        continuity.source_session_id.as_deref(),
        Some("remote-task-1")
    );
    assert!(continuity.summary_text.contains("ok"));
    assert_eq!(continuity.history_window["schema_version"], json!(1));

    let run_after_complete = manager.get_run(&run.id).await.expect("get run");
    assert_eq!(run_after_complete.status, TeamRunStatus::Completed);
    assert!(run_after_complete.ended_at.is_some());

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let step_working_event = events
        .iter()
        .find(|event| event.event_type == "step_working")
        .expect("step_working event should exist");
    assert_eq!(
        step_working_event.payload["runtime_handle_id"],
        json!("remote-task-1")
    );
    assert!(
        step_working_event.payload.get("remote_task_id").is_none(),
        "step_working payload should not expose legacy remote_task_id"
    );
    let event_types: Vec<&str> = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect();
    assert_eq!(
        event_types,
        vec![
            "run_submitted",
            "step_submitted",
            "run_working",
            "step_working",
            "step_completed",
            "continuity_state_updated",
            "run_completed"
        ]
    );
}

#[tokio::test]
async fn complete_step_offloads_large_output_to_workspace_context_artifact() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!("agenthub-context-artifact-{unique_suffix}"));
    std::fs::create_dir_all(&workspace).expect("create workspace directory");
    let workspace_text = workspace.to_string_lossy().to_string();
    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind("planner")
    .bind("planner")
    .bind(&workspace_text)
    .bind("codex")
    .bind("[]")
    .bind("use_existing")
    .bind("manual")
    .bind("idle")
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert planner agent");

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "artifact-team".to_string(),
            description: Some("team with large continuity output".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-artifact"), json!({"payload":"start"}))
        .await
        .expect("create run");
    let step = manager
        .submit_step(
            &run.id,
            "large_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"emit large output"})),
        )
        .await
        .expect("submit step");
    let _ = manager
        .start_step(&step.id, Some("remote-task-artifact"))
        .await
        .expect("start step");

    let large_text = "x".repeat(12_000);
    manager
        .complete_step(
            &step.id,
            Some(json!({
                "summary":"large payload",
                "details": large_text,
                "api_key":"secret-value"
            })),
        )
        .await
        .expect("complete step");

    let continuity = manager
        .get_member_continuity_state(&team.id, "planner")
        .await
        .expect("get continuity state")
        .expect("continuity state should exist");
    let pointer = continuity
        .history_window
        .get("artifact_pointer")
        .expect("artifact pointer should exist for oversized output");
    let pointer_path = pointer
        .get("path")
        .and_then(|value| value.as_str())
        .expect("artifact pointer path should be string");
    assert!(
        pointer_path.starts_with(&format!(".cache/context/run/{}/artifact-", run.id)),
        "unexpected pointer path: {pointer_path}"
    );

    let artifact_row = sqlx::query(
        r#"
        SELECT artifact_path, artifact_size_bytes
        FROM team_context_artifacts
        WHERE run_id = ?1 AND member_id = ?2
        ORDER BY artifact_seq DESC
        LIMIT 1
        "#,
    )
    .bind(&run.id)
    .bind("planner")
    .fetch_one(&db)
    .await
    .expect("fetch context artifact row");
    let artifact_path: String = artifact_row.get("artifact_path");
    let artifact_size: i64 = artifact_row.get("artifact_size_bytes");
    assert!(artifact_size > 0);
    assert!(
        std::path::Path::new(&artifact_path).exists(),
        "artifact path should exist: {artifact_path}"
    );
    let artifact_content =
        std::fs::read_to_string(&artifact_path).expect("read persisted artifact content");
    assert!(
        artifact_content.contains("[redacted]"),
        "sensitive keys should be redacted in persisted artifact"
    );
    let state_path = workspace.join(".cache/context/state.md");
    let state_text = std::fs::read_to_string(&state_path).expect("read runtime state snapshot");
    let note_relative_path = format!(
        ".cache/context/run/{}/continuity.md",
        continuity.source_run_id
    );
    let note_path = workspace
        .join(".cache/context/run")
        .join(&continuity.source_run_id)
        .join("continuity.md");
    let note_text = std::fs::read_to_string(&note_path).expect("read runtime continuity note");
    assert!(state_text.contains("# Team Runtime State"));
    assert!(state_text.contains("- schema_family: team_runtime_state"));
    assert!(state_text.contains("- schema_version: 1"));
    assert!(state_text.contains("- team_id:"));
    assert!(state_text.contains("- member_id: planner"));
    assert!(state_text.contains("- current_execution_run_id:"));
    assert!(state_text.contains("- continuity_mode: inherit_recent"));
    assert!(state_text.contains(format!("- continuity_note_path: {note_relative_path}").as_str()));
    assert!(state_text.contains(pointer_path));
    assert!(note_text.contains("# Team Continuity Note"));
    assert!(note_text.contains("- schema_family: team_continuity_note"));
    assert!(note_text.contains("- schema_version: 1"));
    assert!(note_text.contains("- current_execution_run_id:"));
    assert!(note_text.contains("- continuity_source_execution_run_id:"));
    assert!(note_text.contains("## Summary"));
    assert!(note_text.contains("large payload"));
    assert!(note_text.contains("## History Window"));

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let continuity_event = events
        .iter()
        .find(|event| event.event_type == "continuity_state_updated")
        .expect("continuity_state_updated event should exist");
    assert_eq!(
        continuity_event.payload["source_runtime_handle_id"],
        json!("remote-task-artifact")
    );
    assert!(
        continuity_event.payload.get("source_session_id").is_none(),
        "continuity_state_updated should not expose legacy source_session_id"
    );
    assert_eq!(
        continuity_event.payload["artifact_offload_status"],
        json!("persisted")
    );
    assert!(
        continuity_event.payload.get("artifact_pointer").is_some(),
        "continuity_state_updated should include artifact pointer metadata"
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn complete_step_keeps_success_when_runtime_state_snapshot_write_fails() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    let workspace =
        std::env::temp_dir().join(format!("agenthub-context-state-write-fail-{unique_suffix}"));
    std::fs::create_dir_all(workspace.join(".cache/context"))
        .expect("create workspace context directory");
    std::fs::create_dir_all(workspace.join(".cache/context/state.md"))
        .expect("create conflicting state snapshot path");
    let workspace_text = workspace.to_string_lossy().to_string();
    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind("planner")
    .bind("planner")
    .bind(&workspace_text)
    .bind("codex")
    .bind("[]")
    .bind("use_existing")
    .bind("manual")
    .bind("idle")
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert planner agent");

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "state-write-fail-team".to_string(),
            description: Some("team with conflicting runtime state path".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-write-fail"), json!({"payload":"start"}))
        .await
        .expect("create run");
    let step = manager
        .submit_step(
            &run.id,
            "state_write_fail_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"exercise best-effort state snapshot write"})),
        )
        .await
        .expect("submit step");
    let _ = manager
        .start_step(&step.id, Some("remote-task-state-write-fail"))
        .await
        .expect("start step");

    let completed = manager
        .complete_step(
            &step.id,
            Some(json!({
                "summary":"best effort snapshot",
                "details":"state snapshot should not block completion"
            })),
        )
        .await
        .expect("complete step should succeed despite snapshot write failure");

    assert_eq!(completed.status, TeamStepStatus::Completed);
    let continuity = manager
        .get_member_continuity_state(&team.id, "planner")
        .await
        .expect("get continuity state")
        .expect("continuity state should exist");
    assert_eq!(continuity.summary_text, "best effort snapshot");

    let run_after_complete = manager.get_run(&run.id).await.expect("get run");
    assert_eq!(run_after_complete.status, TeamRunStatus::Completed);
    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn complete_step_offloads_large_output_to_leader_runtime_workspace_context_artifact() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    let workspace =
        std::env::temp_dir().join(format!("agenthub-leader-context-artifact-{unique_suffix}"));
    std::fs::create_dir_all(&workspace).expect("create workspace directory");
    let workspace_text = workspace.to_string_lossy().to_string();
    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind("planner")
    .bind("planner")
    .bind(&workspace_text)
    .bind("codex")
    .bind("[]")
    .bind("use_existing")
    .bind("manual")
    .bind("idle")
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert planner agent");

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "leader-artifact-team".to_string(),
            description: Some("team with leader continuity output".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner","role":"leader"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-artifact"), json!({"payload":"start"}))
        .await
        .expect("create run");
    let step = manager
        .submit_step(
            &run.id,
            "large_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"emit large output"})),
        )
        .await
        .expect("submit step");
    let _ = manager
        .start_step(&step.id, Some("remote-task-artifact"))
        .await
        .expect("start step");

    manager
        .complete_step(
            &step.id,
            Some(json!({
                "summary":"large payload",
                "details": "x".repeat(12_000),
                "api_key":"secret-value"
            })),
        )
        .await
        .expect("complete step");

    let artifact_row = sqlx::query(
        r#"
        SELECT artifact_path
        FROM team_context_artifacts
        WHERE run_id = ?1 AND member_id = ?2
        ORDER BY artifact_seq DESC
        LIMIT 1
        "#,
    )
    .bind(&run.id)
    .bind("planner")
    .fetch_one(&db)
    .await
    .expect("fetch context artifact row");
    let artifact_path: String = artifact_row.get("artifact_path");
    let expected_runtime_workdir = derive_team_runtime_workdir(
        &workspace_text,
        &AcpActorSkillContext {
            team_id: Some(team.id.clone()),
            current_run_id: None,
            actor_id: "planner".to_string(),
            default_channel: DEFAULT_ACTOR_CHANNEL.to_string(),
            member_role: Some("leader".to_string()),
            member_skills: Vec::new(),
            contract_version: None,
            continuity: None,
        },
        &WorktreeMode::UseExisting,
    );
    let expected_prefix = std::path::Path::new(&expected_runtime_workdir)
        .join(".cache/context/run")
        .to_string_lossy()
        .to_string();
    assert!(
        artifact_path.starts_with(&expected_prefix),
        "artifact path should be under derived leader runtime workspace: {artifact_path}"
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn flush_run_context_persists_artifact_and_then_noops_with_checkpoint() {
    let db = setup_test_db().await;
    let event_dbs = AgentEventDbRouter::new(std::env::temp_dir().join(format!(
        "agenthub-team-flush-eventdb-{}",
        uuid::Uuid::new_v4()
    )));
    let manager = TeamManager::new_with_event_dbs(db.clone(), event_dbs.clone());

    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!("agenthub-memory-flush-{unique_suffix}"));
    std::fs::create_dir_all(&workspace).expect("create workspace directory");
    let workspace_text = workspace.to_string_lossy().to_string();
    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind("planner")
    .bind("planner")
    .bind(&workspace_text)
    .bind("codex")
    .bind("[]")
    .bind("use_existing")
    .bind("manual")
    .bind("running")
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert planner agent");

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "flush-team".to_string(),
            description: Some("team with flushable context".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-flush"), json!({"payload":"start"}))
        .await
        .expect("create run");
    let step = manager
        .submit_step(
            &run.id,
            "flush_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"flush"})),
        )
        .await
        .expect("submit step");
    let _ = manager
        .start_step(&step.id, Some("session-flush-1"))
        .await
        .expect("start step");

    let event_db = event_dbs
        .pool_for_agent("planner")
        .await
        .expect("open planner event db");
    sqlx::query(
        r#"
        INSERT INTO agent_events (session_id, seq, ts, stream, message)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
    )
    .bind("session-flush-1")
    .bind("1")
    .bind(100_i64)
    .bind("acp")
    .bind(r#"{"type":"agent_message","content":"first signal","api_key":"secret"}"#)
    .execute(&event_db)
    .await
    .expect("insert first agent event");
    sqlx::query(
        r#"
        INSERT INTO agent_events (session_id, seq, ts, stream, message)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
    )
    .bind("session-flush-1")
    .bind("2")
    .bind(101_i64)
    .bind("system")
    .bind("plain text event")
    .execute(&event_db)
    .await
    .expect("insert second agent event");

    let first = manager
        .flush_run_context(
            &run.id,
            crate::team::TeamMemoryFlushRequest {
                member_id: "planner".to_string(),
                session_id: None,
                trigger: "manual".to_string(),
                max_events: None,
            },
        )
        .await
        .expect("flush context first time");
    assert_eq!(first.status, "persisted");
    assert_eq!(first.flushed_events, 2);
    assert!(first.artifact_pointer.is_some());
    assert_eq!(first.reason, None);

    let checkpoint_event_id: i64 = sqlx::query_scalar(
        r#"
        SELECT last_event_id
        FROM team_context_flush_checkpoint
        WHERE run_id = ?1 AND member_id = ?2 AND session_id = ?3
        "#,
    )
    .bind(&run.id)
    .bind("planner")
    .bind("session-flush-1")
    .fetch_one(&db)
    .await
    .expect("fetch checkpoint");
    assert!(checkpoint_event_id > 0);

    let second = manager
        .flush_run_context(
            &run.id,
            crate::team::TeamMemoryFlushRequest {
                member_id: "planner".to_string(),
                session_id: Some("session-flush-1".to_string()),
                trigger: "manual".to_string(),
                max_events: None,
            },
        )
        .await
        .expect("flush context second time");
    assert_eq!(second.status, "noop");
    assert_eq!(second.reason.as_deref(), Some("no_new_events"));
    assert_eq!(second.flushed_events, 0);

    let events = manager
        .list_run_events(&run.id, 200, None)
        .await
        .expect("list run events");
    let event_types = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect::<Vec<_>>();
    assert!(
        event_types.contains(&"memory_flush_started"),
        "memory_flush_started event should be recorded"
    );
    assert!(
        event_types.contains(&"memory_flush_persisted"),
        "memory_flush_persisted event should be recorded"
    );
    assert!(
        event_types.contains(&"memory_flush_noop"),
        "memory_flush_noop event should be recorded"
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn flush_run_context_fails_when_session_mapping_missing() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "flush-missing-session-team".to_string(),
            description: Some("team with no session mapping".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-missing-session"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let result = manager
        .flush_run_context(
            &run.id,
            crate::team::TeamMemoryFlushRequest {
                member_id: "planner".to_string(),
                session_id: None,
                trigger: "manual".to_string(),
                max_events: None,
            },
        )
        .await
        .expect("flush context should return failed result");
    assert_eq!(result.status, "failed");
    assert_eq!(result.reason.as_deref(), Some("session_mapping_missing"));
    assert!(result.artifact_pointer.is_none());

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let event_types = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect::<Vec<_>>();
    assert!(
        event_types.contains(&"memory_flush_started"),
        "memory_flush_started event should be recorded"
    );
    assert!(
        event_types.contains(&"memory_flush_failed"),
        "memory_flush_failed event should be recorded"
    );
}

#[tokio::test]
async fn input_required_and_resume_transitions_update_run_and_emit_events() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "input-required-team".to_string(),
            description: Some("team requiring manual input".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-input"), json!({"payload":"start"}))
        .await
        .expect("create run");
    let step = manager
        .submit_step(
            &run.id,
            "input_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"collect feedback"})),
        )
        .await
        .expect("submit step");
    let _ = manager
        .start_step(&step.id, Some("remote-task-input"))
        .await
        .expect("start step");

    let input_required = manager
        .set_step_input_required(
            &step.id,
            Some("approval is required"),
            Some(json!({"question":"approve?"})),
        )
        .await
        .expect("set input required");
    assert_eq!(input_required.status, TeamStepStatus::InputRequired);
    assert_eq!(
        input_required.error_text.as_deref(),
        Some("approval is required")
    );
    assert_eq!(
        input_required.input,
        Some(json!({"goal":"collect feedback","question":"approve?"}))
    );

    let run_after_input_required = manager.get_run(&run.id).await.expect("get run");
    assert_eq!(
        run_after_input_required.status,
        TeamRunStatus::InputRequired
    );

    let resumed = manager
        .resume_step(&step.id, Some(json!({"answer":"approved"})))
        .await
        .expect("resume step");
    assert_eq!(resumed.status, TeamStepStatus::Working);
    assert!(resumed.error_text.is_none());
    assert_eq!(
        resumed.input,
        Some(json!({
            "goal":"collect feedback",
            "question":"approve?",
            "answer":"approved"
        }))
    );

    let run_after_resume = manager.get_run(&run.id).await.expect("get run");
    assert_eq!(run_after_resume.status, TeamRunStatus::Working);

    let _ = manager
        .complete_step(&step.id, Some(json!({"result":"done"})))
        .await
        .expect("complete step");
    let run_after_complete = manager.get_run(&run.id).await.expect("get run");
    assert_eq!(run_after_complete.status, TeamRunStatus::Completed);

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let step_resumed_event = events
        .iter()
        .find(|event| event.event_type == "step_resumed")
        .expect("step_resumed event should exist");
    assert_eq!(
        step_resumed_event.payload["runtime_handle_id"],
        json!("remote-task-input")
    );
    assert!(
        step_resumed_event.payload.get("remote_task_id").is_none(),
        "step_resumed payload should not expose legacy remote_task_id"
    );

    let continuity_event = events
        .iter()
        .find(|event| event.event_type == "continuity_state_updated")
        .expect("continuity_state_updated event should exist");
    assert_eq!(
        continuity_event.payload["source_runtime_handle_id"],
        json!("remote-task-input")
    );
    assert!(
        continuity_event.payload.get("source_session_id").is_none(),
        "continuity_state_updated should not expose legacy source_session_id"
    );

    let event_types: Vec<&str> = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect();
    assert_eq!(
        event_types,
        vec![
            "run_submitted",
            "step_submitted",
            "run_working",
            "step_working",
            "run_input_required",
            "step_input_required",
            "run_working",
            "step_resumed",
            "step_completed",
            "continuity_state_updated",
            "run_completed"
        ]
    );
}

#[tokio::test]
async fn reconcile_loop_step_tracks_round_state_and_events() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "reconcile-round-team".to_string(),
            description: Some("team for reconcile round tracking".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"leader"},
                    {"member_id":"worker","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let (task, _) = manager
        .create_task(
            &team.id,
            "Reconcile task",
            "planner",
            json!({
                "execution_plan": {
                    "steps": [{
                        "step_key":"worker-implement",
                        "member_id":"worker",
                        "goal":"finish the patch",
                        "acceptance":["tests pass"],
                        "execution":{"mode":"reconcile_loop","max_rounds":3}
                    }]
                }
            }),
            "group_chat",
            Some("reconcile"),
        )
        .await
        .expect("create task");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-reconcile"),
            json!({"task_id": task.id, "prompt":"execute reconcile step"}),
        )
        .await
        .expect("create run");
    let step = manager
        .list_steps(&run.id)
        .await
        .expect("list steps")
        .into_iter()
        .next()
        .expect("materialized step");

    let started = manager
        .start_step(&step.id, Some("session-reconcile"))
        .await
        .expect("start reconcile step");
    assert_eq!(
        started.input,
        Some(json!({
            "task_execution_step": {
                "goal":"finish the patch",
                "acceptance":["tests pass"],
                "execution":{"mode":"reconcile_loop","max_rounds":3},
                "round_state":{
                    "current_round":1,
                    "latest_status":"working"
                }
            }
        }))
    );

    let waiting = manager
        .set_step_input_required(
            &step.id,
            Some("need review"),
            Some(json!({"question":"approve?"})),
        )
        .await
        .expect("input required");
    assert_eq!(
        waiting.input,
        Some(json!({
            "task_execution_step": {
                "goal":"finish the patch",
                "acceptance":["tests pass"],
                "execution":{"mode":"reconcile_loop","max_rounds":3},
                "round_state":{
                    "current_round":1,
                    "latest_status":"input_required",
                    "latest_outcome":"input_required",
                    "latest_summary":"need review"
                }
            },
            "question":"approve?"
        }))
    );

    let resumed = manager
        .resume_step(&step.id, Some(json!({"answer":"approved"})))
        .await
        .expect("resume step");
    assert_eq!(
        resumed.input,
        Some(json!({
            "task_execution_step": {
                "goal":"finish the patch",
                "acceptance":["tests pass"],
                "execution":{"mode":"reconcile_loop","max_rounds":3},
                "round_state":{
                    "current_round":2,
                    "latest_status":"working"
                }
            },
            "question":"approve?",
            "answer":"approved"
        }))
    );

    let completed = manager
        .complete_step(
            &step.id,
            Some(json!({"summary":"patch is merge-ready","result":"done"})),
        )
        .await
        .expect("complete step");
    assert_eq!(
        completed.input,
        Some(json!({
            "task_execution_step": {
                "goal":"finish the patch",
                "acceptance":["tests pass"],
                "execution":{"mode":"reconcile_loop","max_rounds":3},
                "round_state":{
                    "current_round":2,
                    "latest_status":"completed",
                    "latest_outcome":"completed",
                    "latest_summary":"patch is merge-ready"
                }
            },
            "question":"approve?",
            "answer":"approved"
        }))
    );

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list events");
    let reconcile_events = events
        .iter()
        .filter(|event| event.event_type.starts_with("step_reconcile_round_"))
        .map(|event| (event.event_type.as_str(), event.payload.clone()))
        .collect::<Vec<_>>();
    assert_eq!(reconcile_events.len(), 4);
    assert_eq!(reconcile_events[0].0, "step_reconcile_round_started");
    assert_eq!(reconcile_events[0].1["round"], json!(1));
    assert_eq!(reconcile_events[1].0, "step_reconcile_round_finished");
    assert_eq!(reconcile_events[1].1["status"], json!("input_required"));
    assert_eq!(reconcile_events[2].0, "step_reconcile_round_started");
    assert_eq!(reconcile_events[2].1["round"], json!(2));
    assert_eq!(reconcile_events[3].0, "step_reconcile_round_finished");
    assert_eq!(reconcile_events[3].1["status"], json!("completed"));
}

#[tokio::test]
async fn continue_step_advances_reconcile_round_without_leader_resume() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "continue-reconcile-team".to_string(),
            description: Some("team for reconcile continue".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"leader"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let (task, _) = manager
        .create_task(
            &team.id,
            "Continue reconcile task",
            "planner",
            json!({
                "execution_plan": {
                    "steps": [{
                        "step_key":"worker-implement",
                        "member_id":"worker-1",
                        "goal":"finish the patch",
                        "acceptance":["tests pass"],
                        "execution":{"mode":"reconcile_loop","max_rounds":3}
                    }]
                }
            }),
            "group_chat",
            Some("continue-reconcile"),
        )
        .await
        .expect("create task");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-continue-reconcile"),
            json!({"task_id": task.id, "prompt":"execute reconcile continue"}),
        )
        .await
        .expect("create run");
    let step = manager
        .list_steps(&run.id)
        .await
        .expect("list steps")
        .into_iter()
        .next()
        .expect("materialized step");

    let started = manager
        .start_step(&step.id, Some("session-reconcile"))
        .await
        .expect("start reconcile step");
    assert_eq!(started.status, TeamStepStatus::Working);

    let continued = manager
        .continue_step(
            &step.id,
            Some(json!({"summary":"tests still failing on lint","artifact":"round-1.log"})),
        )
        .await
        .expect("continue reconcile step");
    assert_eq!(continued.status, TeamStepStatus::Working);
    assert_eq!(
        continued.input,
        Some(json!({
            "task_execution_step": {
                "goal":"finish the patch",
                "acceptance":["tests pass"],
                "execution":{"mode":"reconcile_loop","max_rounds":3},
                "round_state":{
                    "current_round":2,
                    "latest_status":"working"
                }
            }
        }))
    );
    assert_eq!(
        continued.output,
        Some(json!({"summary":"tests still failing on lint","artifact":"round-1.log"}))
    );

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let event_types = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        event_types,
        vec![
            "run_submitted",
            "step_submitted",
            "run_working",
            "step_working",
            "step_reconcile_round_started",
            "step_continued",
            "step_reconcile_round_finished",
            "step_reconcile_round_started",
        ]
    );
    let continue_event = events
        .iter()
        .find(|event| event.event_type == "step_continued")
        .expect("step_continued event");
    assert_eq!(continue_event.payload["continued_from_round"], json!(1));
    assert_eq!(continue_event.payload["continued_to_round"], json!(2));
    assert_eq!(
        continue_event.payload["summary"],
        json!("tests still failing on lint")
    );
    let reconcile_events = events
        .iter()
        .filter(|event| event.event_type.starts_with("step_reconcile_round_"))
        .map(|event| (event.event_type.as_str(), event.payload.clone()))
        .collect::<Vec<_>>();
    assert_eq!(reconcile_events.len(), 3);
    assert_eq!(reconcile_events[0].1["round"], json!(1));
    assert_eq!(reconcile_events[1].1["round"], json!(1));
    assert_eq!(reconcile_events[1].1["status"], json!("continued"));
    assert_eq!(reconcile_events[2].1["round"], json!(2));
}

#[tokio::test]
async fn continue_step_rejects_reconcile_loop_after_max_rounds() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "continue-reconcile-max-rounds-team".to_string(),
            description: Some("team for reconcile max rounds".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"leader"},
                    {"member_id":"worker-1","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let (task, _) = manager
        .create_task(
            &team.id,
            "Continue reconcile max rounds task",
            "planner",
            json!({
                "execution_plan": {
                    "steps": [{
                        "step_key":"worker-implement",
                        "member_id":"worker-1",
                        "goal":"finish the patch",
                        "acceptance":["tests pass"],
                        "execution":{"mode":"reconcile_loop","max_rounds":1}
                    }]
                }
            }),
            "group_chat",
            Some("continue-reconcile-max-rounds"),
        )
        .await
        .expect("create task");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-continue-reconcile-max-rounds"),
            json!({"task_id": task.id, "prompt":"execute reconcile continue"}),
        )
        .await
        .expect("create run");
    let step = manager
        .list_steps(&run.id)
        .await
        .expect("list steps")
        .into_iter()
        .next()
        .expect("materialized step");

    manager
        .start_step(&step.id, Some("session-reconcile"))
        .await
        .expect("start reconcile step");
    let err = manager
        .continue_step(&step.id, Some(json!({"summary":"still working"})))
        .await
        .expect_err("continue step should reject at max rounds");
    assert!(
        err.to_string().contains("max_rounds=1"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn continue_step_persists_reconcile_round_result_artifact() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!(
        "agenthub-reconcile-continue-artifact-{unique_suffix}"
    ));
    std::fs::create_dir_all(&workspace).expect("create workspace directory");
    let workspace_text = workspace.to_string_lossy().to_string();
    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind("worker")
    .bind("Worker Agent")
    .bind(&workspace_text)
    .bind("codex")
    .bind("[]")
    .bind("use_existing")
    .bind("manual")
    .bind("idle")
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert worker agent");

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "continue-round-artifact-team".to_string(),
            description: Some("team for reconcile round artifact persistence".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"leader"},
                    {"member_id":"worker","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let (task, _) = manager
        .create_task(
            &team.id,
            "Round artifact task",
            "planner",
            json!({
                "execution_plan": {
                    "steps": [{
                        "step_key":"worker-implement",
                        "member_id":"worker",
                        "goal":"finish the patch",
                        "acceptance":["tests pass"],
                        "execution":{"mode":"reconcile_loop","max_rounds":3}
                    }]
                }
            }),
            "group_chat",
            Some("round-artifact"),
        )
        .await
        .expect("create task");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-round-artifact"),
            json!({"task_id": task.id, "prompt":"execute reconcile continue"}),
        )
        .await
        .expect("create run");
    let step = manager
        .list_steps(&run.id)
        .await
        .expect("list steps")
        .into_iter()
        .next()
        .expect("materialized step");

    manager
        .start_step(&step.id, Some("session-reconcile"))
        .await
        .expect("start reconcile step");
    manager
        .continue_step(
            &step.id,
            Some(json!({
                "summary":"tests still failing on lint",
                "artifacts":["logs/round-1.txt"]
            })),
        )
        .await
        .expect("continue reconcile step");

    let artifact_row = sqlx::query(
        r#"
        SELECT artifact_path, artifact_size_bytes
        FROM team_context_artifacts
        WHERE run_id = ?1 AND member_id = ?2 AND artifact_kind = ?3
        ORDER BY artifact_seq DESC
        LIMIT 1
        "#,
    )
    .bind(&run.id)
    .bind("worker")
    .bind("reconcile_round_result")
    .fetch_one(&db)
    .await
    .expect("fetch reconcile round artifact row");
    let artifact_path: String = artifact_row.get("artifact_path");
    assert!(artifact_row.get::<i64, _>("artifact_size_bytes") > 0);
    let artifact_content =
        std::fs::read_to_string(&artifact_path).expect("read persisted artifact content");
    assert!(artifact_content.contains("\"status\":\"continued\""));
    assert!(artifact_content.contains("\"round\":1"));
    assert!(artifact_content.contains("tests still failing on lint"));
    assert!(
        artifact_path.starts_with(
            &workspace
                .join(".cache/context/run")
                .to_string_lossy()
                .to_string()
        ),
        "artifact path should be under worker runtime workspace: {artifact_path}"
    );

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let continue_event = events
        .iter()
        .find(|event| event.event_type == "step_continued")
        .expect("step_continued event");
    assert_eq!(
        continue_event.payload["artifact_offload_status"],
        json!("persisted")
    );
    assert!(continue_event.payload.get("artifact_pointer").is_some());
    assert!(
        continue_event.payload.get("output").is_none(),
        "step_continued should rely on artifact pointer instead of echoing full output"
    );
    let round_finished_event = events
        .iter()
        .find(|event| {
            event.event_type == "step_reconcile_round_finished"
                && event.payload["status"] == json!("continued")
        })
        .expect("continued round event");
    assert_eq!(
        round_finished_event.payload["artifact_offload_status"],
        json!("persisted")
    );
    assert!(
        round_finished_event
            .payload
            .get("artifact_pointer")
            .is_some()
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn input_required_persists_reconcile_round_result_artifact() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    let workspace =
        std::env::temp_dir().join(format!("agenthub-reconcile-input-artifact-{unique_suffix}"));
    std::fs::create_dir_all(&workspace).expect("create workspace directory");
    let workspace_text = workspace.to_string_lossy().to_string();
    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind("worker")
    .bind("Worker Agent")
    .bind(&workspace_text)
    .bind("codex")
    .bind("[]")
    .bind("use_existing")
    .bind("manual")
    .bind("idle")
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert worker agent");

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "input-round-artifact-team".to_string(),
            description: Some("team for reconcile input artifact persistence".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"leader"},
                    {"member_id":"worker","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let (task, _) = manager
        .create_task(
            &team.id,
            "Input artifact task",
            "planner",
            json!({
                "execution_plan": {
                    "steps": [{
                        "step_key":"worker-implement",
                        "member_id":"worker",
                        "goal":"request review",
                        "acceptance":["review granted"],
                        "execution":{"mode":"reconcile_loop","max_rounds":3}
                    }]
                }
            }),
            "group_chat",
            Some("input-artifact"),
        )
        .await
        .expect("create task");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-input-artifact"),
            json!({"task_id": task.id, "prompt":"execute reconcile input-required"}),
        )
        .await
        .expect("create run");
    let step = manager
        .list_steps(&run.id)
        .await
        .expect("list steps")
        .into_iter()
        .next()
        .expect("materialized step");

    manager
        .start_step(&step.id, Some("session-reconcile"))
        .await
        .expect("start reconcile step");
    manager
        .set_step_input_required(
            &step.id,
            Some("need human review"),
            Some(json!({"question":"approve?"})),
        )
        .await
        .expect("mark input required");

    let artifact_row = sqlx::query(
        r#"
        SELECT artifact_path
        FROM team_context_artifacts
        WHERE run_id = ?1 AND member_id = ?2 AND artifact_kind = ?3
        ORDER BY artifact_seq DESC
        LIMIT 1
        "#,
    )
    .bind(&run.id)
    .bind("worker")
    .bind("reconcile_round_result")
    .fetch_one(&db)
    .await
    .expect("fetch reconcile round artifact row");
    let artifact_path: String = artifact_row.get("artifact_path");
    let artifact_content =
        std::fs::read_to_string(&artifact_path).expect("read persisted artifact content");
    assert!(artifact_content.contains("\"status\":\"input_required\""));
    assert!(artifact_content.contains("need human review"));
    assert!(artifact_content.contains("\"question\":\"approve?\""));

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let input_required_event = events
        .iter()
        .find(|event| event.event_type == "step_input_required")
        .expect("step_input_required event");
    assert_eq!(
        input_required_event.payload["artifact_offload_status"],
        json!("persisted")
    );
    assert!(
        input_required_event
            .payload
            .get("artifact_pointer")
            .is_some()
    );
    let round_finished_event = events
        .iter()
        .find(|event| {
            event.event_type == "step_reconcile_round_finished"
                && event.payload["status"] == json!("input_required")
        })
        .expect("input_required round event");
    assert_eq!(
        round_finished_event.payload["artifact_offload_status"],
        json!("persisted")
    );
    assert!(
        round_finished_event
            .payload
            .get("artifact_pointer")
            .is_some()
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn list_steps_returns_sorted_steps_for_a_run() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "list-steps-team".to_string(),
            description: Some("team for step listing".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-list"), json!({"payload":"list"}))
        .await
        .expect("create run");
    let run_2 = manager
        .create_run(&team.id, Some("ctx-list-2"), json!({"payload":"list-2"}))
        .await
        .expect("create second run");

    let _ = manager
        .submit_step(
            &run.id,
            "z-step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"z"})),
        )
        .await
        .expect("submit z step");
    let _ = manager
        .submit_step(
            &run.id,
            "a-step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"a"})),
        )
        .await
        .expect("submit a step");
    let _ = manager
        .submit_step(
            &run_2.id,
            "other-run-step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"other"})),
        )
        .await
        .expect("submit step in other run");

    let listed = manager.list_steps(&run.id).await.expect("list steps");
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].run_id, run.id);
    assert_eq!(listed[1].run_id, run.id);
    assert_eq!(
        listed
            .iter()
            .map(|step| step.step_key.as_str())
            .collect::<Vec<_>>(),
        vec!["a-step", "z-step"]
    );
}

#[tokio::test]
async fn actor_messages_support_inbox_and_ack_flow() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-message-team".to_string(),
            description: Some("team for actor message flow".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-msg"), json!({"payload":"start"}))
        .await
        .expect("create run");

    let sent = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({"text":"please review"}),
            idempotency_key: None,
        })
        .await
        .expect("send message");
    assert_eq!(sent.status, TeamActorMessageStatus::Pending);
    assert_eq!(sent.transport, TeamActorMessageTransport::Local);
    assert_eq!(sent.payload, json!({"text":"please review"}));
    assert_eq!(sent.from_actor_kind, ActorIdentityKind::Agent);
    assert_eq!(sent.to_actor_kind, ActorIdentityKind::Agent);

    let human_sent = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "user:alice",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({"text":"human request"}),
            idempotency_key: None,
        })
        .await
        .expect("send human message");
    assert_eq!(human_sent.from_actor_kind, ActorIdentityKind::Human);
    assert_eq!(human_sent.to_actor_kind, ActorIdentityKind::Agent);

    let pending_inbox = manager
        .list_actor_inbox(&run.id, "reviewer", 100, None, false)
        .await
        .expect("list pending inbox");
    assert_eq!(pending_inbox.len(), 2);
    let pending_ids = pending_inbox
        .iter()
        .map(|message| message.message_id)
        .collect::<Vec<_>>();
    assert!(pending_ids.contains(&sent.message_id));
    assert!(pending_ids.contains(&human_sent.message_id));
    let human_inbox = pending_inbox
        .iter()
        .find(|message| message.message_id == human_sent.message_id)
        .expect("human message in inbox");
    assert_eq!(human_inbox.from_actor_kind, ActorIdentityKind::Human);
    assert_eq!(human_inbox.to_actor_kind, ActorIdentityKind::Agent);

    let delivered = manager
        .ack_actor_message(&run.id, "reviewer", sent.message_id)
        .await
        .expect("ack message");
    assert!(delivered.status_changed);
    assert_eq!(delivered.message.status, TeamActorMessageStatus::Delivered);
    assert!(delivered.message.delivered_at.is_some());

    let pending_after_ack = manager
        .list_actor_inbox(&run.id, "reviewer", 100, None, false)
        .await
        .expect("list pending after ack");
    assert_eq!(pending_after_ack.len(), 1);
    assert_eq!(pending_after_ack[0].message_id, human_sent.message_id);
    assert_eq!(
        pending_after_ack[0].from_actor_kind,
        ActorIdentityKind::Human
    );

    let inbox_with_delivered = manager
        .list_actor_inbox(&run.id, "reviewer", 100, None, true)
        .await
        .expect("list inbox with delivered");
    assert_eq!(inbox_with_delivered.len(), 2);
    let delivered_message = inbox_with_delivered
        .iter()
        .find(|message| message.message_id == sent.message_id)
        .expect("delivered message exists");
    assert_eq!(delivered_message.status, TeamActorMessageStatus::Delivered);

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let event_types: Vec<&str> = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect();
    assert_eq!(
        event_types,
        vec![
            "run_submitted",
            "actor_message_sent",
            "actor_message_sent",
            "actor_message_delivered"
        ]
    );
}

#[tokio::test]
async fn actor_messages_detect_pending_payload_type_by_actor_inbox() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db);

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-message-payload-type-team".to_string(),
            description: Some("team for payload type pending lookup".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-msg-type"), json!({"payload":"start"}))
        .await
        .expect("create run");

    let first = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({"type":"chat_message","text":"first"}),
            idempotency_key: None,
        })
        .await
        .expect("send first message");
    let second = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({"type":"chat_message","text":"second"}),
            idempotency_key: None,
        })
        .await
        .expect("send second message");
    let _other_type = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({"type":"  worker_status  ","status":"done"}),
            idempotency_key: None,
        })
        .await
        .expect("send other type message");

    let has_other_chat_pending = manager
        .has_pending_actor_message_payload_type(
            &run.id,
            "reviewer",
            "chat_message",
            Some(second.message_id),
        )
        .await
        .expect("check chat pending excluding latest");
    assert!(has_other_chat_pending);

    let has_prior_for_first_chat = manager
        .has_pending_actor_message_payload_type(
            &run.id,
            "reviewer",
            "chat_message",
            Some(first.message_id),
        )
        .await
        .expect("check chat pending before first");
    assert!(!has_prior_for_first_chat);

    manager
        .ack_actor_message(&run.id, "reviewer", first.message_id)
        .await
        .expect("ack first");
    let still_has_other_chat_pending = manager
        .has_pending_actor_message_payload_type(
            &run.id,
            "reviewer",
            "chat_message",
            Some(second.message_id),
        )
        .await
        .expect("check chat pending after ack");
    assert!(!still_has_other_chat_pending);

    let has_worker_status_pending = manager
        .has_pending_actor_message_payload_type(&run.id, "reviewer", "worker_status", None)
        .await
        .expect("check worker_status pending");
    assert!(has_worker_status_pending);
}

#[tokio::test]
async fn actor_ack_reports_noop_when_message_is_already_delivered() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db);

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-ack-noop-team".to_string(),
            description: Some("team for duplicate ack diagnostics".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-ack-noop"), json!({"payload":"start"}))
        .await
        .expect("create run");
    let sent = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({"type":"chat_message","text":"please review"}),
            idempotency_key: None,
        })
        .await
        .expect("send message");

    let first = manager
        .ack_actor_message(&run.id, "reviewer", sent.message_id)
        .await
        .expect("first ack");
    assert!(first.status_changed);
    assert_eq!(first.message.status, TeamActorMessageStatus::Delivered);

    let second = manager
        .ack_actor_message(&run.id, "reviewer", sent.message_id)
        .await
        .expect("second ack");
    assert!(!second.status_changed);
    assert_eq!(second.message.status, TeamActorMessageStatus::Delivered);
}

#[tokio::test]
async fn actor_mailbox_service_returns_contract_responses() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-service-team".to_string(),
            description: Some("team for actor mailbox service contract".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-msg-service"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let sent = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: Some("reviewer".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({"text":"please review"}),
            idempotency_key: Some("msg-service-1".to_string()),
        })
        .await
        .expect("actor send");
    assert_eq!(sent.state, TeamActorMessageStatus::Pending);
    assert!(!sent.deduped);

    let deduped = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: Some("reviewer".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({"text":"please review"}),
            idempotency_key: Some("msg-service-1".to_string()),
        })
        .await
        .expect("actor send deduped");
    assert_eq!(sent.message_id, deduped.message_id);
    assert!(deduped.deduped);

    let inbox = service
        .actor_inbox(ActorInboxRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(50),
            states: None,
        })
        .await
        .expect("actor inbox");
    assert_eq!(inbox.messages.len(), 1);
    assert_eq!(inbox.messages[0].message_id, sent.message_id);
    assert_eq!(inbox.next_cursor, Some(sent.message_id));
    assert_eq!(inbox.pending_count, 1);

    let acked = service
        .actor_ack(ActorAckRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            message_id: sent.message_id,
            ack_token: None,
            result: None,
        })
        .await
        .expect("actor ack");
    assert_eq!(acked.message_id, sent.message_id);
    assert_eq!(acked.state, TeamActorMessageStatus::Delivered);
}

#[tokio::test]
async fn actor_mailbox_service_cursor_can_hide_page_messages_without_resetting_pending_count() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-cursor-team".to_string(),
            description: Some("team for actor inbox cursor semantics".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-msg-cursor"), json!({"payload":"start"}))
        .await
        .expect("create run");

    let sent = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: Some("reviewer".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({"text":"please review"}),
            idempotency_key: Some("msg-cursor-1".to_string()),
        })
        .await
        .expect("actor send");

    let inbox = service
        .actor_inbox(ActorInboxRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            cursor: Some(sent.message_id),
            limit: Some(50),
            states: None,
        })
        .await
        .expect("actor inbox with cursor");
    assert!(inbox.messages.is_empty());
    assert_eq!(inbox.next_cursor, None);
    assert_eq!(inbox.pending_count, 1);
}

#[tokio::test]
async fn actor_mailbox_service_channel_send_broadcasts_and_preserves_mentions() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-channel-team".to_string(),
            description: Some("team for channel mailbox broadcast".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner"},
                    {"member_id":"reviewer"},
                    {"member_id":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-channel-mailbox"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let sent = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: None,
            channel_id: Some("all".to_string()),
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"@reviewer please validate api contract"
            }),
            idempotency_key: Some("msg-channel-mailbox-1".to_string()),
        })
        .await
        .expect("send channel mailbox message");
    assert_eq!(sent.state, TeamActorMessageStatus::Pending);

    let rows = sqlx::query(
        r#"
        SELECT to_actor_id, payload_json
        FROM team_actor_messages
        WHERE run_id = ?1
        ORDER BY id ASC
        "#,
    )
    .bind(&run.id)
    .fetch_all(&db)
    .await
    .expect("load channel mailbox rows");
    assert_eq!(rows.len(), 2);
    let recipients = rows
        .iter()
        .map(|row| row.get::<String, _>("to_actor_id"))
        .collect::<Vec<_>>();
    assert_eq!(
        recipients,
        vec!["reviewer".to_string(), "worker".to_string()]
    );
    for row in &rows {
        let payload: Value = serde_json::from_str(row.get::<String, _>("payload_json").as_str())
            .expect("decode forwarded channel payload");
        assert_eq!(payload["delivery_scope"], json!("channel_broadcast"));
        assert_eq!(payload["channel_id"], json!("all"));
        assert_eq!(payload["team_id"], json!(team.id));
        assert!(
            payload["authority_message_id"]
                .as_i64()
                .is_some_and(|value| value > 0),
            "missing authority_message_id: {payload}"
        );
        assert_eq!(payload["mention_actor_ids"], json!(["reviewer"]));
        assert_eq!(payload["mentioned_actor_ids"], json!(["reviewer"]));
        assert_eq!(
            payload["text"],
            json!("@reviewer please validate api contract")
        );
    }

    let shared_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM team_conversation_messages
        WHERE task_id IN (
            SELECT id
            FROM team_tasks
            WHERE team_id = ?1
              AND (title = 'all' OR trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), '')) = 'shared_thread')
        )
        "#,
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("count shared thread messages");
    assert_eq!(shared_count, 1);

    let replica_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM team_channel_message_replicas")
            .fetch_one(&db)
            .await
            .expect("count channel replica rows");
    assert_eq!(replica_count, 0);
}

#[tokio::test]
async fn actor_mailbox_service_channel_send_honors_explicit_mentions_without_raw_text() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-explicit-mention-team".to_string(),
            description: Some("team for explicit channel mention payloads".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner"},
                    {"member_id":"reviewer"},
                    {"member_id":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-channel-explicit-mention"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: None,
            channel_id: Some("all".to_string()),
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"please review the api contract",
                "mentioned_actor_ids":["reviewer"]
            }),
            idempotency_key: Some("msg-channel-explicit-mention-1".to_string()),
        })
        .await
        .expect("send explicit mention channel message");

    let rows = sqlx::query(
        r#"
        SELECT to_actor_id, payload_json
        FROM team_actor_messages
        WHERE run_id = ?1
        ORDER BY id ASC
        "#,
    )
    .bind(&run.id)
    .fetch_all(&db)
    .await
    .expect("load channel mailbox rows");
    assert_eq!(rows.len(), 2);

    for row in &rows {
        let payload: Value = serde_json::from_str(row.get::<String, _>("payload_json").as_str())
            .expect("decode forwarded payload");
        assert_eq!(payload["mention_actor_ids"], json!(["reviewer"]));
        assert_eq!(payload["mentioned_actor_ids"], json!(["reviewer"]));
    }
}

#[tokio::test]
async fn actor_mailbox_service_channel_send_reuses_canonical_message_on_idempotent_retry() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-channel-idempotent-team".to_string(),
            description: Some("team for channel mailbox idempotency".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner"},
                    {"member_id":"reviewer"},
                    {"member_id":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-channel-mailbox-idempotent"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    for _ in 0..2 {
        service
            .actor_send(ActorSendRequest {
                run_id: run.id.clone(),
                from_actor_id: "planner".to_string(),
                from_peer_id: None,
                to_actor_id: None,
                channel_id: Some("all".to_string()),
                to_peer_id: None,
                channel: Some("coordination".to_string()),
                transport: Some(TeamActorMessageTransport::Local),
                route: None,
                payload: json!({
                    "type":"chat_message",
                    "text":"@reviewer please validate retry behavior"
                }),
                idempotency_key: Some("msg-channel-mailbox-idempotent-1".to_string()),
            })
            .await
            .expect("send channel mailbox message");
    }

    let shared_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM team_conversation_messages
        WHERE task_id IN (
            SELECT id
            FROM team_tasks
            WHERE team_id = ?1
              AND (title = 'all' OR trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), '')) = 'shared_thread')
        )
        "#,
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("count shared thread messages");
    assert_eq!(shared_count, 1);

    let mailbox_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM team_actor_messages
        WHERE run_id = ?1
        "#,
    )
    .bind(&run.id)
    .fetch_one(&db)
    .await
    .expect("count mailbox rows");
    assert_eq!(mailbox_count, 2);
}

#[tokio::test]
async fn actor_mailbox_service_channel_send_auto_routes_remote_recipients_over_p2p() {
    let db = setup_test_db().await;
    sqlx::query("ALTER TABLE agents ADD COLUMN target_node_id TEXT")
        .execute(&db)
        .await
        .expect("add target_node_id");
    sqlx::query(
        r#"
        CREATE TABLE agent_nodes (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            grpc_target TEXT NOT NULL,
            tls_server_name TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )
        "#,
    )
    .execute(&db)
    .await
    .expect("create agent_nodes");

    let manager = TeamManager::new(db.clone());
    manager.configure_internal_grpc_peer_client(Some(InternalGrpcPeerClientConfig {
        shared_secret: "team-channel-p2p-secret".to_string(),
        expected_issuer: Some("agenthub".to_string()),
        expected_audience: Some("agenthub-internal".to_string()),
        source_node_id: "main".to_string(),
        cert_dir: std::env::temp_dir()
            .join(format!("team-channel-p2p-{}", Uuid::new_v4()))
            .to_string_lossy()
            .to_string(),
        security_mode: InternalGrpcSecurityMode::Mtls,
    }));
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-channel-p2p-team".to_string(),
            description: Some("team for channel mailbox p2p broadcast".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner"},
                    {"member_id":"reviewer"},
                    {"member_id":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-channel-mailbox-p2p"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let now = Utc::now().timestamp();
    for (agent_id, target_node_id) in [
        ("planner", None),
        ("reviewer", None),
        ("worker", Some("node-east")),
    ] {
        sqlx::query(
            r#"
            INSERT INTO agents (
                id,
                name,
                workdir,
                command,
                args,
                worktree_mode,
                code_mode,
                agent_loop_enabled,
                source,
                status,
                created_at,
                updated_at,
                target_node_id
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, 'manual', 'running', ?7, ?8, ?9)
            "#,
        )
        .bind(agent_id)
        .bind(format!("Agent {agent_id}"))
        .bind(format!("/tmp/{agent_id}"))
        .bind("agenthub")
        .bind("[]")
        .bind("use_existing")
        .bind(now)
        .bind(now)
        .bind(target_node_id)
        .execute(&db)
        .await
        .expect("insert agent");
    }

    sqlx::query(
        r#"
        INSERT INTO agent_nodes (id, name, grpc_target, tls_server_name, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind("node-east")
    .bind("Node East")
    .bind("https://node-east.internal:50051")
    .bind("node-east.internal")
    .bind(now)
    .bind(now)
    .execute(&db)
    .await
    .expect("insert remote node");

    let sent = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: None,
            channel_id: Some("all".to_string()),
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"@worker please validate remote relay"
            }),
            idempotency_key: Some("msg-channel-mailbox-p2p-1".to_string()),
        })
        .await
        .expect("send p2p channel mailbox message");
    assert_eq!(sent.state, TeamActorMessageStatus::Pending);

    let canonical_row = sqlx::query(
        r#"
        SELECT from_actor_id, to_actor_id, route, payload_json
        FROM team_conversation_messages
        WHERE task_id IN (
            SELECT id
            FROM team_tasks
            WHERE team_id = ?1
              AND (title = 'all' OR trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), '')) = 'shared_thread')
        )
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("load canonical channel conversation message");
    let canonical_payload: Value =
        serde_json::from_str(canonical_row.get::<String, _>("payload_json").as_str())
            .expect("decode canonical channel payload");
    assert_eq!(canonical_row.get::<String, _>("from_actor_id"), "planner");
    assert!(
        canonical_row
            .try_get::<Option<String>, _>("to_actor_id")
            .ok()
            .flatten()
            .is_none()
    );
    assert_eq!(canonical_row.get::<String, _>("route"), "group_chat");
    assert_eq!(
        canonical_payload["text"],
        json!("@worker please validate remote relay")
    );

    let rows = sqlx::query(
        r#"
        SELECT to_actor_id, to_peer_id, transport, route_json, payload_json
        FROM team_actor_messages
        WHERE run_id = ?1
        ORDER BY to_actor_id ASC
        "#,
    )
    .bind(&run.id)
    .fetch_all(&db)
    .await
    .expect("load p2p mailbox rows");
    assert_eq!(rows.len(), 2);

    let reviewer = rows
        .iter()
        .find(|row| row.get::<String, _>("to_actor_id") == "reviewer")
        .expect("reviewer row");
    assert_eq!(reviewer.get::<String, _>("to_peer_id"), ACTOR_MAIN_PEER_ID);
    assert_eq!(reviewer.get::<String, _>("transport"), "local");
    assert!(
        reviewer
            .try_get::<Option<String>, _>("route_json")
            .ok()
            .flatten()
            .is_none()
    );

    let worker = rows
        .iter()
        .find(|row| row.get::<String, _>("to_actor_id") == "worker")
        .expect("worker row");
    assert_eq!(worker.get::<String, _>("to_peer_id"), ACTOR_NODE_PEER_ID);
    assert_eq!(worker.get::<String, _>("transport"), "remote");
    let route: Value =
        serde_json::from_str(worker.get::<String, _>("route_json").as_str()).expect("route");
    assert_eq!(route["kind"], json!("grpc"));
    assert_eq!(
        route["grpc_target"],
        json!("https://node-east.internal:50051")
    );
    assert_eq!(route["tls_server_name"], json!("node-east.internal"));
    assert_eq!(route["target_node_id"], json!("node-east"));
    assert!(
        route.get("access_token").is_none(),
        "persisted route should stay stable and omit access_token: {route}"
    );
    assert!(
        route.get("issued_at").is_none() && route.get("expires_at").is_none(),
        "persisted route should omit transient credential metadata: {route}"
    );

    let worker_payload: Value =
        serde_json::from_str(worker.get::<String, _>("payload_json").as_str())
            .expect("worker payload");
    assert_eq!(worker_payload["delivery_scope"], json!("channel_broadcast"));
    assert_eq!(worker_payload["team_id"], json!(team.id));
    assert!(
        worker_payload["authority_message_id"]
            .as_i64()
            .is_some_and(|value| value > 0),
        "missing authority_message_id: {worker_payload}"
    );
    assert_eq!(worker_payload["mention_actor_ids"], json!(["worker"]));
    assert_eq!(worker_payload["mentioned_actor_ids"], json!(["worker"]));
}

#[tokio::test]
async fn actor_mailbox_service_persists_agent_reply_into_shared_thread() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();
    let mut events = manager.subscribe_conversation_events();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-shared-thread-team".to_string(),
            description: Some("team for canonical shared reply persistence".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-shared-thread"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let before_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM team_conversation_messages WHERE task_id IN (SELECT id FROM team_tasks WHERE team_id = ?1)",
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("count conversation messages before send");
    assert_eq!(before_count, 0);

    service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: Some("user".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"hello human",
                "current_phase":"planning",
                "correlation_id":"corr-1"
            }),
            idempotency_key: Some("msg-shared-thread-1".to_string()),
        })
        .await
        .expect("send shared thread reply");

    let shared_task_row = sqlx::query(
        r#"
        SELECT id, context_json
        FROM team_tasks
        WHERE team_id = ?1
          AND (title = 'all' OR trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), '')) = 'shared_thread')
        LIMIT 1
        "#,
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("load shared thread task");
    let shared_task_id: String = shared_task_row.get("id");
    let shared_task_context: String = shared_task_row.get("context_json");
    let shared_task_context_json: Value =
        serde_json::from_str(&shared_task_context).expect("decode shared task context");
    assert_eq!(
        shared_task_context_json["bootstrap_kind"],
        json!("shared_thread")
    );

    let row = sqlx::query(
        r#"
        SELECT
            from_actor_id,
            to_actor_id,
            route,
            payload_json
        FROM team_conversation_messages
        WHERE task_id = ?1
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .bind(&shared_task_id)
    .fetch_one(&db)
    .await
    .expect("load canonical shared thread message");

    let from_actor_id: String = row.get("from_actor_id");
    let to_actor_id: Option<String> = row.get("to_actor_id");
    let route: String = row.get("route");
    let payload_json: String = row.get("payload_json");
    let payload: Value = serde_json::from_str(&payload_json).expect("decode canonical payload");
    assert_eq!(from_actor_id, "planner");
    assert_eq!(to_actor_id, None);
    assert_eq!(route, "group_chat");
    assert_eq!(payload["type"], json!("chat_message"));
    assert_eq!(payload["text"], json!("hello human"));
    assert_eq!(payload["correlation_id"], json!("corr-1"));
    assert!(payload.get("current_phase").is_none());

    let event = tokio::time::timeout(std::time::Duration::from_millis(500), events.recv())
        .await
        .expect("receive canonical shared thread event")
        .expect("canonical shared thread event result");
    assert_eq!(event.team_id, team.id);
    assert_eq!(event.task_id, shared_task_id);
    assert_eq!(event.message_id, None);
    assert_eq!(event.source, "canonical_chat_reply");
}

#[tokio::test]
async fn actor_mailbox_service_deduped_shared_thread_reply_does_not_duplicate_conversation() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-shared-thread-dedup-team".to_string(),
            description: Some("team for shared reply idempotency".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-shared-thread-dedup"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let first = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: Some("user".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({"type":"chat_message","text":"hello human"}),
            idempotency_key: Some("msg-shared-thread-dedup".to_string()),
        })
        .await
        .expect("first shared thread send");
    let second = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: Some("user".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({"type":"chat_message","text":"hello human"}),
            idempotency_key: Some("msg-shared-thread-dedup".to_string()),
        })
        .await
        .expect("deduped shared thread send");
    assert_eq!(first.message_id, second.message_id);

    let canonical_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM team_conversation_messages
        WHERE task_id IN (
            SELECT id
            FROM team_tasks
            WHERE team_id = ?1
              AND (title = 'all' OR trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), '')) = 'shared_thread')
        )
        "#,
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("count canonical shared thread messages");
    assert_eq!(canonical_count, 1);
}

#[tokio::test]
async fn actor_mailbox_service_does_not_persist_agent_to_agent_chat_into_shared_thread() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-private-chat-team".to_string(),
            description: Some("team for private mailbox reply routing".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-private-chat"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: Some("reviewer".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"internal review request"
            }),
            idempotency_key: Some("msg-private-chat-1".to_string()),
        })
        .await
        .expect("send internal mailbox reply");

    let shared_task_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM team_tasks
        WHERE team_id = ?1
          AND (title = 'all' OR trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), '')) = 'shared_thread')
        "#,
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("count shared thread tasks");
    assert_eq!(shared_task_count, 0);

    let conversation_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM team_conversation_messages WHERE task_id IN (SELECT id FROM team_tasks WHERE team_id = ?1)",
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("count conversation messages after private send");
    assert_eq!(conversation_count, 0);
}

#[tokio::test]
async fn actor_mailbox_service_canonicalizes_stringified_json_reply_into_shared_thread() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-stringified-reply-team".to_string(),
            description: Some("team for stringified shared reply canonicalization".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-stringified-reply"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: Some("user".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!("{\"type\":\"chat_message\",\"text\":\"hello from string\",\"current_phase\":\"planning\",\"correlation_id\":\"corr-string\"}"),
            idempotency_key: Some("msg-stringified-chat-1".to_string()),
        })
        .await
        .expect("send stringified shared reply");

    let payload_json: String = sqlx::query_scalar(
        r#"
        SELECT payload_json
        FROM team_conversation_messages
        WHERE task_id IN (
            SELECT id
            FROM team_tasks
            WHERE team_id = ?1
              AND (title = 'all' OR trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), '')) = 'shared_thread')
        )
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("load canonical payload for stringified reply");
    let payload: Value =
        serde_json::from_str(&payload_json).expect("decode canonical stringified payload");
    assert_eq!(payload["type"], json!("chat_message"));
    assert_eq!(payload["text"], json!("hello from string"));
    assert_eq!(payload["correlation_id"], json!("corr-string"));
    assert!(payload.get("current_phase").is_none());
}

#[tokio::test]
async fn actor_mailbox_service_reuses_existing_shared_thread_for_canonical_reply() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-existing-shared-thread-team".to_string(),
            description: Some("team for shared thread reuse".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-existing-shared-thread"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let (shared_task, _conversation) = manager
        .create_task(
            &team.id,
            "all",
            "user",
            json!({
                "bootstrap_kind":"shared_thread",
                "bootstrap_source":"test"
            }),
            "group_chat",
            Some("shared"),
        )
        .await
        .expect("create existing shared thread");

    service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: Some("user".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"reuse existing thread"
            }),
            idempotency_key: Some("msg-existing-shared-thread-1".to_string()),
        })
        .await
        .expect("send shared thread reply into existing thread");

    let shared_task_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM team_tasks
        WHERE team_id = ?1
          AND (title = 'all' OR trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), '')) = 'shared_thread')
        "#,
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("count shared thread tasks after reuse");
    assert_eq!(shared_task_count, 1);

    let message_task_id: String = sqlx::query_scalar(
        r#"
        SELECT task_id
        FROM team_conversation_messages
        WHERE task_id IN (
            SELECT id
            FROM team_tasks
            WHERE team_id = ?1
              AND (title = 'all' OR trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), '')) = 'shared_thread')
        )
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("load canonical message task id");
    assert_eq!(message_task_id, shared_task.id);
}

#[tokio::test]
async fn actor_mailbox_service_prefers_shared_thread_with_latest_message_when_duplicates_exist() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-canonical-shared-thread-team".to_string(),
            description: Some("team for canonical shared thread selection".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-canonical-shared-thread"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let (preferred_task, _preferred_conversation) = manager
        .create_task(
            &team.id,
            "all",
            "user",
            json!({
                "bootstrap_kind":"shared_thread",
                "bootstrap_source":"test"
            }),
            "group_chat",
            Some("shared"),
        )
        .await
        .expect("create preferred shared thread");
    let (older_task, _older_conversation) = manager
        .create_task(
            &team.id,
            "all",
            "user",
            json!({
                "bootstrap_kind":"shared_thread",
                "bootstrap_source":"test"
            }),
            "group_chat",
            Some("shared"),
        )
        .await
        .expect("create older shared thread");

    manager
        .append_task_conversation_message(
            &older_task.id,
            "user",
            None,
            "group_chat",
            json!({
                "type":"chat_message",
                "text":"older duplicate thread"
            }),
        )
        .await
        .expect("append older duplicate thread message");
    manager
        .append_task_conversation_message(
            &preferred_task.id,
            "user",
            None,
            "group_chat",
            json!({
                "type":"chat_message",
                "text":"newest canonical thread"
            }),
        )
        .await
        .expect("append newest canonical thread message");

    service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: Some("user".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"persist into canonical duplicate thread"
            }),
            idempotency_key: Some("msg-canonical-shared-thread-1".to_string()),
        })
        .await
        .expect("send shared thread reply into canonical duplicate thread");

    let message_task_id: String = sqlx::query_scalar(
        r#"
        SELECT task_id
        FROM team_conversation_messages
        WHERE task_id IN (
            SELECT id
            FROM team_tasks
            WHERE team_id = ?1
              AND (title = 'all' OR trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), '')) = 'shared_thread')
        )
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("load canonical duplicate shared thread message task id");
    assert_eq!(message_task_id, preferred_task.id);
}

#[tokio::test]
async fn actor_mailbox_service_validates_required_fields() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db);
    let service = manager.actor_mailbox_service();

    let err = service
        .actor_send(ActorSendRequest {
            run_id: " ".to_string(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: Some("reviewer".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: None,
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({"text":"invalid"}),
            idempotency_key: None,
        })
        .await
        .expect_err("blank run_id should fail");
    assert_eq!(err.code, ActorServiceErrorCode::BadRequest);
}

#[tokio::test]
async fn actor_message_send_is_idempotent_by_key() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-message-idempotent-team".to_string(),
            description: Some("team for idempotent send flow".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-msg-idempotent"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let first = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({"text":"please review"}),
            idempotency_key: Some("msg-1"),
        })
        .await
        .expect("first send");
    let second = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({"text":"please review"}),
            idempotency_key: Some("msg-1"),
        })
        .await
        .expect("retry send");
    assert_eq!(first.message_id, second.message_id);

    let deduped_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM team_actor_messages
        WHERE run_id = ?1 AND from_actor_id = ?2 AND idempotency_key = ?3
        "#,
    )
    .bind(&run.id)
    .bind("planner")
    .bind("msg-1")
    .fetch_one(&db)
    .await
    .expect("count deduped messages");
    assert_eq!(deduped_count, 1);

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let sent_count = events
        .iter()
        .filter(|event| event.event_type == "actor_message_sent")
        .count();
    assert_eq!(sent_count, 1);
}

#[tokio::test]
async fn actor_message_send_rejects_mismatched_payload_for_same_idempotency_key() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-message-idempotency-conflict-team".to_string(),
            description: Some("team for idempotency conflict flow".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-msg-idempotency-conflict"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let _ = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({"text":"please review"}),
            idempotency_key: Some("msg-1"),
        })
        .await
        .expect("first send");
    let err = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({"text":"changed payload"}),
            idempotency_key: Some("msg-1"),
        })
        .await
        .expect_err("mismatched payload should conflict");
    assert!(
        TeamManager::is_actor_message_idempotency_conflict(&err),
        "expected idempotency conflict error, got: {err}"
    );

    let message_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM team_actor_messages WHERE run_id = ?1 AND from_actor_id = ?2",
    )
    .bind(&run.id)
    .bind("planner")
    .fetch_one(&db)
    .await
    .expect("count actor messages");
    assert_eq!(message_count, 1);
}

#[tokio::test]
async fn remote_actor_messages_relay_success_marks_message_delivered() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let (endpoint, captures, server_handle) = spawn_relay_http_server(StatusCode::OK).await;

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-relay-success-team".to_string(),
            description: Some("team for relay success flow".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-relay-success"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let sent = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "remote-reviewer",
            to_peer_id: ACTOR_NODE_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Remote,
            route: Some(json!({
                "endpoint": endpoint,
                "method": "POST",
                "headers": {
                    "x-agenthub-relay-test": "success"
                },
                "auth": {
                    "type": "bearer",
                    "token": "relay-token"
                },
                "signing": {
                    "type": "hmac_sha256",
                    "secret": "relay-signing-secret",
                    "header": "x-agenthub-signature",
                    "timestamp_header": "x-agenthub-timestamp"
                }
            })),
            payload: json!({"text":"review this"}),
            idempotency_key: None,
        })
        .await
        .expect("send remote message");
    assert_eq!(sent.status, TeamActorMessageStatus::Pending);

    let relay_result = manager
        .relay_remote_messages_once(100, 3, 30)
        .await
        .expect("relay remote messages");
    assert_eq!(relay_result.scanned, 1);
    assert_eq!(relay_result.delivered, 1);
    assert_eq!(relay_result.retried, 0);
    assert_eq!(relay_result.dead_lettered, 0);

    let relayed_row = sqlx::query(
        r#"
        SELECT status, delivered_at
        FROM team_actor_messages
        WHERE id = ?1
        "#,
    )
    .bind(sent.message_id)
    .fetch_one(&db)
    .await
    .expect("fetch relayed message row");
    let relayed_status: String = relayed_row.get("status");
    let relayed_at: Option<i64> = relayed_row.try_get("delivered_at").ok();
    assert_eq!(relayed_status, "delivered");
    assert!(relayed_at.is_some());

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let delivered_count = events
        .iter()
        .filter(|event| event.event_type == "actor_message_delivered")
        .count();
    assert_eq!(delivered_count, 1);

    let captured = captures.lock().await;
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].method, "POST");
    assert_eq!(
        captured[0].headers.get("x-agenthub-relay-test"),
        Some(&"success".to_string())
    );
    assert_eq!(
        captured[0].headers.get("authorization"),
        Some(&"Bearer relay-token".to_string())
    );
    assert!(
        captured[0].headers.contains_key("x-agenthub-signature"),
        "missing signature header"
    );
    assert!(
        captured[0].headers.contains_key("x-agenthub-timestamp"),
        "missing signature timestamp header"
    );
    assert_eq!(
        captured[0].headers.get("x-agenthub-message-id"),
        Some(&sent.message_id.to_string())
    );
    assert_eq!(captured[0].body["run_id"], run.id);
    assert_eq!(captured[0].body["source_node_id"], "main");
    assert_eq!(captured[0].body["target_node_id"], "node");
    assert_eq!(captured[0].body["from_actor_id"], "planner");
    assert_eq!(captured[0].body["from_actor_kind"], "agent");
    assert_eq!(captured[0].body["to_actor_id"], "remote-reviewer");
    assert_eq!(captured[0].body["to_actor_kind"], "agent");
    assert_eq!(captured[0].body["scope"], json!(["node:p2p"]));
    assert_eq!(captured[0].body["kid"], "phase1-shared-key");
    assert!(captured[0].body["payload_digest"].is_string());
    assert_eq!(captured[0].body["payload"]["text"], "review this");
    drop(captured);
    server_handle.abort();
}

#[tokio::test]
async fn remote_actor_messages_relay_supports_retry_and_dead_letter() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let (retry_endpoint, retry_captures, retry_server_handle) =
        spawn_relay_http_server(StatusCode::SERVICE_UNAVAILABLE).await;
    let (dead_endpoint, dead_captures, dead_server_handle) =
        spawn_relay_http_server(StatusCode::BAD_REQUEST).await;

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-relay-policy-team".to_string(),
            description: Some("team for relay retry/dead-letter policy".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-relay-policy"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let retry_message = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "remote-retry",
            to_peer_id: ACTOR_NODE_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Remote,
            route: Some(json!({
                "endpoint": retry_endpoint,
                "method": "POST",
                "auth": {
                    "type": "header",
                    "name": "x-agenthub-auth",
                    "value": "retry-secret"
                }
            })),
            payload: json!({"text":"retry this"}),
            idempotency_key: None,
        })
        .await
        .expect("send retry remote message");
    let dead_message = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "remote-dead",
            to_peer_id: ACTOR_NODE_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Remote,
            route: Some(json!({
                "endpoint": dead_endpoint,
                "method": "POST"
            })),
            payload: json!({"text":"dead-letter this"}),
            idempotency_key: None,
        })
        .await
        .expect("send dead remote message");

    let relay_result = manager
        .relay_remote_messages_once(100, 3, 60)
        .await
        .expect("relay remote messages");
    assert_eq!(relay_result.scanned, 2);
    assert_eq!(relay_result.delivered, 0);
    assert_eq!(relay_result.retried, 1);
    assert_eq!(relay_result.dead_lettered, 1);

    let retry_row = sqlx::query(
        r#"
        SELECT status, relay_attempt, relay_next_retry_at, dead_letter_at
        FROM team_actor_messages
        WHERE id = ?1
        "#,
    )
    .bind(retry_message.message_id)
    .fetch_one(&db)
    .await
    .expect("fetch retry message row");
    let retry_status: String = retry_row.get("status");
    let retry_attempt: i64 = retry_row.get("relay_attempt");
    let retry_next: Option<i64> = retry_row
        .try_get("relay_next_retry_at")
        .expect("retry next retry at");
    let retry_dead_letter_at: Option<i64> = retry_row
        .try_get("dead_letter_at")
        .expect("retry dead letter at");
    assert_eq!(retry_status, "pending");
    assert_eq!(retry_attempt, 1);
    assert!(retry_next.is_some());
    assert!(retry_dead_letter_at.is_none());

    let dead_row = sqlx::query(
        r#"
        SELECT status, relay_attempt, relay_next_retry_at, dead_letter_at
        FROM team_actor_messages
        WHERE id = ?1
        "#,
    )
    .bind(dead_message.message_id)
    .fetch_one(&db)
    .await
    .expect("fetch dead-letter message row");
    let dead_status: String = dead_row.get("status");
    let dead_attempt: i64 = dead_row.get("relay_attempt");
    let dead_next: Option<i64> = dead_row
        .try_get("relay_next_retry_at")
        .expect("dead next retry at");
    let dead_dead_letter_at: Option<i64> = dead_row
        .try_get("dead_letter_at")
        .expect("dead dead letter at");
    assert_eq!(dead_status, "dead_letter");
    assert_eq!(dead_attempt, 1);
    assert!(dead_next.is_none());
    assert!(dead_dead_letter_at.is_some());

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let retry_event_count = events
        .iter()
        .filter(|event| event.event_type == "actor_message_relay_retry")
        .count();
    let dead_letter_event_count = events
        .iter()
        .filter(|event| event.event_type == "actor_message_dead_letter")
        .count();
    assert_eq!(retry_event_count, 1);
    assert_eq!(dead_letter_event_count, 1);

    let retry_captured = retry_captures.lock().await;
    assert_eq!(retry_captured.len(), 1);
    assert_eq!(
        retry_captured[0].headers.get("x-agenthub-auth"),
        Some(&"retry-secret".to_string())
    );
    assert_eq!(retry_captured[0].body["to_actor_id"], "remote-retry");
    drop(retry_captured);

    let dead_captured = dead_captures.lock().await;
    assert_eq!(dead_captured.len(), 1);
    assert_eq!(dead_captured[0].body["to_actor_id"], "remote-dead");
    drop(dead_captured);

    retry_server_handle.abort();
    dead_server_handle.abort();
}

#[tokio::test]
async fn remote_actor_messages_relay_rejects_invalid_header_values() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let (endpoint, captures, server_handle) = spawn_relay_http_server(StatusCode::OK).await;

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-relay-invalid-header-team".to_string(),
            description: Some("team for invalid relay header validation".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-relay-invalid-header"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let sent = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "planner",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "remote-reviewer",
            to_peer_id: ACTOR_NODE_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Remote,
            route: Some(json!({
                "endpoint": endpoint,
                "method": "POST",
                "headers": {
                    "x-agenthub-relay-test": "bad\nvalue"
                }
            })),
            payload: json!({"text":"review this"}),
            idempotency_key: None,
        })
        .await
        .expect("send remote message");
    assert_eq!(sent.status, TeamActorMessageStatus::Pending);

    let relay_result = manager
        .relay_remote_messages_once(100, 3, 30)
        .await
        .expect("relay remote messages");
    assert_eq!(relay_result.scanned, 1);
    assert_eq!(relay_result.delivered, 0);
    assert_eq!(relay_result.retried, 0);
    assert_eq!(relay_result.dead_lettered, 1);

    let rows = sqlx::query(
        r#"
        SELECT status, relay_last_error
        FROM team_actor_messages
        WHERE id = ?1
        "#,
    )
    .bind(sent.message_id)
    .fetch_one(&db)
    .await
    .expect("fetch relay row");
    let status: String = rows.get("status");
    let relay_last_error: Option<String> = rows.try_get("relay_last_error").ok();
    assert_eq!(status, "dead_letter");
    assert!(
        relay_last_error
            .as_deref()
            .is_some_and(|text| text.contains("invalid")),
        "unexpected relay error: {:?}",
        relay_last_error
    );

    let captured = captures.lock().await;
    assert!(captured.is_empty());
    drop(captured);
    server_handle.abort();
}

#[tokio::test]
async fn run_completes_only_after_all_steps_complete() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "multi-step-team".to_string(),
            description: Some("team with two parallel steps".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"},{"member_id":"reviewer"}]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-multi"), json!({"payload":"start"}))
        .await
        .expect("create run");

    let step_1 = manager
        .submit_step(
            &run.id,
            "plan_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"draft"})),
        )
        .await
        .expect("submit step 1");
    let step_2 = manager
        .submit_step(
            &run.id,
            "review_step",
            "reviewer",
            vec!["plan_step".to_string()],
            Some(json!({"goal":"review"})),
        )
        .await
        .expect("submit step 2");

    let _ = manager
        .start_step(&step_1.id, Some("remote-task-1"))
        .await
        .expect("start step 1");
    let _ = manager
        .start_step(&step_2.id, Some("remote-task-2"))
        .await
        .expect("start step 2");

    let _ = manager
        .complete_step(&step_1.id, Some(json!({"result":"done-1"})))
        .await
        .expect("complete step 1");
    let run_after_first_complete = manager.get_run(&run.id).await.expect("get run");
    assert_eq!(run_after_first_complete.status, TeamRunStatus::Working);
    assert!(run_after_first_complete.ended_at.is_none());

    let _ = manager
        .complete_step(&step_2.id, Some(json!({"result":"done-2"})))
        .await
        .expect("complete step 2");
    let run_after_second_complete = manager.get_run(&run.id).await.expect("get run");
    assert_eq!(run_after_second_complete.status, TeamRunStatus::Completed);
    assert!(run_after_second_complete.ended_at.is_some());

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let run_completed_count = events
        .iter()
        .filter(|event| event.event_type == "run_completed")
        .count();
    assert_eq!(run_completed_count, 1);
    assert_eq!(
        events.last().map(|event| event.event_type.as_str()),
        Some("run_completed")
    );
}

#[tokio::test]
async fn fail_step_updates_status_and_emits_event() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "fail-step-team".to_string(),
            description: Some("team with failure".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-fail"), json!({"payload":"start"}))
        .await
        .expect("create run");
    let step = manager
        .submit_step(
            &run.id,
            "failing_step",
            "planner",
            Vec::new(),
            Some(json!({"goal":"can fail"})),
        )
        .await
        .expect("submit step");

    let _ = manager
        .start_step(&step.id, Some("remote-task-fail"))
        .await
        .expect("start step");
    let failed = manager
        .fail_step(&step.id, "remote task failed")
        .await
        .expect("fail step");
    assert_eq!(failed.status, TeamStepStatus::Failed);
    assert_eq!(failed.error_text.as_deref(), Some("remote task failed"));

    let run_after_fail = manager.get_run(&run.id).await.expect("get run");
    assert_eq!(run_after_fail.status, TeamRunStatus::Failed);
    assert!(run_after_fail.ended_at.is_some());

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let event_types: Vec<&str> = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect();
    assert_eq!(
        event_types,
        vec![
            "run_submitted",
            "step_submitted",
            "run_working",
            "step_working",
            "step_failed",
            "run_failed"
        ]
    );
}

#[tokio::test]
async fn list_active_runs_returns_non_terminal_runs_only() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "active-runs-team".to_string(),
            description: Some("team to verify active run listing".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let submitted_run = manager
        .create_run(
            &team.id,
            Some("ctx-submitted"),
            json!({"payload":"submitted"}),
        )
        .await
        .expect("create submitted run");

    let canceled_run = manager
        .create_run(
            &team.id,
            Some("ctx-canceled"),
            json!({"payload":"canceled"}),
        )
        .await
        .expect("create canceled run");
    let _ = manager
        .cancel_run(&canceled_run.id)
        .await
        .expect("cancel run");

    let working_run = manager
        .create_run(&team.id, Some("ctx-working"), json!({"payload":"working"}))
        .await
        .expect("create working run");
    let working_step = manager
        .submit_step(
            &working_run.id,
            "work",
            "planner",
            Vec::new(),
            Some(json!({"goal":"start"})),
        )
        .await
        .expect("submit working step");
    let _ = manager
        .start_step(&working_step.id, Some("remote-working"))
        .await
        .expect("start working step");

    let active_runs = manager
        .list_active_runs(100)
        .await
        .expect("list active runs");
    let active_ids: Vec<&str> = active_runs.iter().map(|run| run.id.as_str()).collect();
    assert!(active_ids.contains(&submitted_run.id.as_str()));
    assert!(active_ids.contains(&working_run.id.as_str()));
    assert!(!active_ids.contains(&canceled_run.id.as_str()));
}

#[tokio::test]
async fn list_active_runs_for_team_excludes_shared_thread_mailbox_runs() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "active-runs-team-filtered".to_string(),
            description: Some("team to verify per-team active run listing".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let visible_run = manager
        .create_run(&team.id, Some("ctx-visible"), json!({"payload":"visible"}))
        .await
        .expect("create visible run");
    let shared_mailbox_run = manager
        .ensure_shared_thread_mailbox_run(&team.id, "shared-thread-task", "conversation-all")
        .await
        .expect("create shared mailbox run");
    sqlx::query(
        "UPDATE team_runs SET status = 'working', started_at = COALESCE(started_at, ?1) WHERE id = ?2",
    )
        .bind(chrono::Utc::now().timestamp())
        .bind(&shared_mailbox_run.id)
        .execute(&db)
        .await
        .expect("promote shared mailbox run to active status");

    let active_runs = manager
        .list_active_runs_for_team(&team.id, 20)
        .await
        .expect("list active runs for team");
    let active_ids: Vec<&str> = active_runs.iter().map(|run| run.id.as_str()).collect();
    assert_eq!(active_ids, vec![visible_run.id.as_str()]);
}

#[tokio::test]
async fn cancel_active_runs_on_startup_requires_manual_restart() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "startup-cancel-team".to_string(),
            description: Some("team to verify startup active-run cancellation".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let submitted_run = manager
        .create_run(
            &team.id,
            Some("ctx-startup-submitted"),
            json!({"payload":"submitted"}),
        )
        .await
        .expect("create submitted run");
    let working_run = manager
        .create_run(
            &team.id,
            Some("ctx-startup-working"),
            json!({"payload":"working"}),
        )
        .await
        .expect("create working run");
    let working_step = manager
        .submit_step(
            &working_run.id,
            "work",
            "planner",
            Vec::new(),
            Some(json!({"goal":"start"})),
        )
        .await
        .expect("submit working step");
    let _ = manager
        .start_step(&working_step.id, Some("remote-startup-working"))
        .await
        .expect("start working step");

    let canceled_count = manager
        .cancel_active_runs_on_startup()
        .await
        .expect("cancel active runs on startup");
    assert_eq!(canceled_count, 2);

    let submitted_after = manager
        .get_run(&submitted_run.id)
        .await
        .expect("get submitted run after startup cancel");
    assert_eq!(submitted_after.status, TeamRunStatus::Canceled);

    let working_after = manager
        .get_run(&working_run.id)
        .await
        .expect("get working run after startup cancel");
    assert_eq!(working_after.status, TeamRunStatus::Canceled);

    let working_step_after = manager
        .get_step(&working_step.id)
        .await
        .expect("get working step after startup cancel");
    assert_eq!(working_step_after.status, TeamStepStatus::Canceled);

    let active_after = manager
        .list_active_runs(100)
        .await
        .expect("list active runs after startup cancel");
    assert!(active_after.is_empty());

    let startup_events = manager
        .list_run_events(&working_run.id, 200, None)
        .await
        .expect("list working run events")
        .into_iter()
        .filter(|event| event.event_type == "run_startup_canceled")
        .collect::<Vec<_>>();
    assert_eq!(startup_events.len(), 1);
}

#[tokio::test]
async fn cancel_active_runs_on_startup_reopens_linked_tasks() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "startup-linked-task-team".to_string(),
            description: Some("team to verify startup cancel reopens linked tasks".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Restart-safe linked task",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("startup-linked-task"),
        )
        .await
        .expect("create task");
    assert_eq!(task.status, TeamTaskStatus::Open);

    let run = manager
        .create_run(
            &team.id,
            Some(task.id.as_str()),
            json!({"task_id": task.id, "payload":"linked"}),
        )
        .await
        .expect("create linked run");
    assert_eq!(run.status, TeamRunStatus::Submitted);

    let in_progress_task = manager.get_task(&task.id).await.expect("reload task");
    assert_eq!(in_progress_task.status, TeamTaskStatus::InProgress);

    let canceled_count = manager
        .cancel_active_runs_on_startup()
        .await
        .expect("cancel active runs on startup");
    assert_eq!(canceled_count, 1);

    let run_after = manager.get_run(&run.id).await.expect("reload canceled run");
    assert_eq!(run_after.status, TeamRunStatus::Canceled);

    let reopened_task = manager
        .get_task(&task.id)
        .await
        .expect("reload reopened task");
    assert_eq!(reopened_task.status, TeamTaskStatus::Open);
    assert_eq!(reopened_task.assigned_member_id, None);
}

#[tokio::test]
async fn resume_run_handles_active_terminal_and_completed_statuses() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db);

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "resume-run-team".to_string(),
            description: Some("team to verify run resume strategy".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let submitted_run = manager
        .create_run(
            &team.id,
            Some("ctx-resume-submitted"),
            json!({"payload":"submitted"}),
        )
        .await
        .expect("create submitted run");
    let resumed_submitted = manager
        .resume_run(&submitted_run.id)
        .await
        .expect("resume submitted run");
    assert_eq!(resumed_submitted.id, submitted_run.id);

    let failed_run = manager
        .create_run(
            &team.id,
            Some("ctx-resume-failed"),
            json!({"payload":"failed"}),
        )
        .await
        .expect("create failed run");
    let failed_step = manager
        .submit_step(
            &failed_run.id,
            "step_failed",
            "planner",
            Vec::new(),
            Some(json!({"goal":"fail"})),
        )
        .await
        .expect("submit failed step");
    let _ = manager
        .start_step(&failed_step.id, Some("remote-failed"))
        .await
        .expect("start failed step");
    let _ = manager
        .fail_step(&failed_step.id, "forced fail")
        .await
        .expect("fail step");
    let resumed_failed = manager
        .resume_run(&failed_run.id)
        .await
        .expect("resume failed run");
    assert_ne!(resumed_failed.id, failed_run.id);
    assert_eq!(resumed_failed.team_id, failed_run.team_id);
    assert_eq!(resumed_failed.context_id, failed_run.context_id);
    assert_eq!(resumed_failed.input, failed_run.input);
    assert_eq!(resumed_failed.status, TeamRunStatus::Submitted);
    let failed_after_resume = manager
        .get_run(&failed_run.id)
        .await
        .expect("get original failed run");
    assert_eq!(failed_after_resume.status, TeamRunStatus::Failed);

    let canceled_run = manager
        .create_run(
            &team.id,
            Some("ctx-resume-canceled"),
            json!({"payload":"canceled"}),
        )
        .await
        .expect("create canceled run");
    let _ = manager
        .cancel_run(&canceled_run.id)
        .await
        .expect("cancel run");
    let resumed_canceled = manager
        .resume_run(&canceled_run.id)
        .await
        .expect("resume canceled run");
    assert_ne!(resumed_canceled.id, canceled_run.id);
    assert_eq!(resumed_canceled.context_id, canceled_run.context_id);
    assert_eq!(resumed_canceled.input, canceled_run.input);
    assert_eq!(resumed_canceled.status, TeamRunStatus::Submitted);

    let completed_run = manager
        .create_run(
            &team.id,
            Some("ctx-resume-completed"),
            json!({"payload":"completed"}),
        )
        .await
        .expect("create completed run");
    let completed_step = manager
        .submit_step(
            &completed_run.id,
            "step_completed",
            "planner",
            Vec::new(),
            Some(json!({"goal":"done"})),
        )
        .await
        .expect("submit completed step");
    let _ = manager
        .start_step(&completed_step.id, Some("remote-completed"))
        .await
        .expect("start completed step");
    let _ = manager
        .complete_step(&completed_step.id, Some(json!({"ok":true})))
        .await
        .expect("complete step");
    let err = manager
        .resume_run(&completed_run.id)
        .await
        .expect_err("completed run should reject resume");
    assert_eq!(
        err.downcast_ref::<TeamRunResumeError>(),
        Some(&TeamRunResumeError::CompletedRun),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn restart_run_creates_new_submission_with_same_context_and_input() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db);

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "restart-run-team".to_string(),
            description: Some("team to verify run restart strategy".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let run = manager
        .create_run(
            &team.id,
            Some("ctx-restart"),
            json!({"payload":"restart-me"}),
        )
        .await
        .expect("create source run");
    let restarted = manager.restart_run(&run.id).await.expect("restart run");

    assert_ne!(restarted.id, run.id);
    assert_eq!(restarted.team_id, run.team_id);
    assert_eq!(restarted.context_id, run.context_id);
    assert_eq!(restarted.input, run.input);
    assert_eq!(restarted.status, TeamRunStatus::Submitted);

    let events = manager
        .list_run_events(&restarted.id, 10, None)
        .await
        .expect("list restarted run events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "run_submitted");
}

#[tokio::test]
async fn list_runs_supports_status_filter_and_cursor() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "list-runs-team".to_string(),
            description: Some("team to verify run listing".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let first_run = manager
        .create_run(&team.id, Some("ctx-list-runs-1"), json!({"seq": 1}))
        .await
        .expect("create first run");
    let second_run = manager
        .create_run(&team.id, Some("ctx-list-runs-2"), json!({"seq": 2}))
        .await
        .expect("create second run");
    let _ = manager
        .cancel_run(&first_run.id)
        .await
        .expect("cancel first run");
    let shared_thread_run = manager
        .ensure_shared_thread_mailbox_run(&team.id, "shared-thread-task", "conversation-all")
        .await
        .expect("create hidden shared thread mailbox run");

    sqlx::query("UPDATE team_runs SET created_at = ?1 WHERE id = ?2")
        .bind(100_i64)
        .bind(&first_run.id)
        .execute(&db)
        .await
        .expect("set first run created_at");
    sqlx::query("UPDATE team_runs SET created_at = ?1 WHERE id = ?2")
        .bind(200_i64)
        .bind(&second_run.id)
        .execute(&db)
        .await
        .expect("set second run created_at");
    sqlx::query("UPDATE team_runs SET created_at = ?1 WHERE id = ?2")
        .bind(300_i64)
        .bind(&shared_thread_run.id)
        .execute(&db)
        .await
        .expect("set shared thread run created_at");

    let all_runs = manager
        .list_runs(&team.id, 100, None, None)
        .await
        .expect("list all runs");
    assert_eq!(all_runs.len(), 2);
    assert_eq!(all_runs[0].id, second_run.id);
    assert_eq!(all_runs[0].summary, None);
    assert_eq!(all_runs[1].id, first_run.id);
    assert_eq!(
        all_runs[1].summary.as_deref(),
        Some("Run was canceled before completion.")
    );

    let canceled_runs = manager
        .list_runs(&team.id, 100, Some("canceled"), None)
        .await
        .expect("list canceled runs");
    assert_eq!(canceled_runs.len(), 1);
    assert_eq!(canceled_runs[0].id, first_run.id);

    let cursor_runs = manager
        .list_runs(&team.id, 100, None, Some(200))
        .await
        .expect("list runs with cursor");
    assert_eq!(cursor_runs.len(), 1);
    assert_eq!(cursor_runs[0].id, first_run.id);

    let limited_runs = manager
        .list_runs(&team.id, 1, None, None)
        .await
        .expect("list limited visible runs");
    assert_eq!(limited_runs.len(), 1);
    assert_eq!(limited_runs[0].id, second_run.id);

    let limited_cursor_runs = manager
        .list_runs(&team.id, 1, None, Some(200))
        .await
        .expect("list limited cursor visible runs");
    assert_eq!(limited_cursor_runs.len(), 1);
    assert_eq!(limited_cursor_runs[0].id, first_run.id);

    let hidden_run = manager
        .get_latest_run_for_task(&team.id, "shared-thread-task")
        .await
        .expect("load hidden shared thread run")
        .expect("hidden shared thread run should exist");
    assert_eq!(hidden_run.id, shared_thread_run.id);
    assert_eq!(
        hidden_run.input["bootstrap_kind"],
        Value::from("shared_thread_mailbox")
    );
}

#[tokio::test]
async fn ensure_shared_thread_mailbox_run_is_idempotent() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "shared-thread-mailbox-idempotent-team".to_string(),
            description: Some("team to verify shared thread mailbox idempotency".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let first = manager
        .ensure_shared_thread_mailbox_run(&team.id, "shared-thread-task", "conversation-all")
        .await
        .expect("create first shared thread mailbox run");
    let second = manager
        .ensure_shared_thread_mailbox_run(&team.id, "shared-thread-task", "conversation-all")
        .await
        .expect("reuse shared thread mailbox run");

    assert_eq!(first.id, second.id);

    let run_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM team_runs
        WHERE team_id = ?1
          AND trim(COALESCE(json_extract(input_json, '$.bootstrap_kind'), '')) = 'shared_thread_mailbox'
          AND trim(COALESCE(json_extract(input_json, '$.task_id'), '')) = 'shared-thread-task'
        "#,
    )
    .bind(&team.id)
    .fetch_one(&db)
    .await
    .expect("count shared thread mailbox runs");
    assert_eq!(run_count, 1);

    let event_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM team_run_events WHERE run_id = ?1")
            .bind(&first.id)
            .fetch_one(&db)
            .await
            .expect("count shared thread mailbox run events");
    assert_eq!(event_count, 2);
}

#[tokio::test]
async fn describe_run_members_returns_live_roster_and_session_state() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "describe-run-members-team".to_string(),
            description: Some("team to verify run member roster".to_string()),
            spec: json!({
                "entrypoint":"leader",
                "members":[
                    {"member_id":"leader","role":"leader","description":"Lead planner"},
                    {"member_id":"worker","role":"worker","description":"Implements changes"}
                ]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-team-members"), json!({"prompt":"go"}))
        .await
        .expect("create run");
    let leader_step = manager
        .submit_step(&run.id, "leader_plan", "leader", Vec::new(), None)
        .await
        .expect("submit leader step");
    let worker_step = manager
        .submit_step(
            &run.id,
            "worker_exec",
            "worker",
            vec!["leader_plan".to_string()],
            None,
        )
        .await
        .expect("submit worker step");

    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind("leader")
    .bind("Leader Agent")
    .bind("/tmp/leader")
    .bind("codex")
    .bind("[]")
    .bind("use_existing")
    .bind("manual")
    .bind("running")
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert leader agent");

    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind("worker")
    .bind("Worker Agent")
    .bind("/tmp/worker")
    .bind("codex")
    .bind("[]")
    .bind("create_worktree")
    .bind("manual")
    .bind("idle")
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert worker agent");

    sqlx::query(
        r#"
        INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
        VALUES (?1, ?2, ?3, ?4, NULL)
        "#,
    )
    .bind("session-leader")
    .bind("leader")
    .bind("running")
    .bind(10_i64)
    .execute(&db)
    .await
    .expect("insert leader session");

    manager
        .start_step(&leader_step.id, Some("session-leader"))
        .await
        .expect("start leader step");

    sqlx::query(
        r#"
        INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
        VALUES (?1, ?2, ?3, ?4, NULL)
        "#,
    )
    .bind("session-worker")
    .bind("worker")
    .bind("running")
    .bind(11_i64)
    .execute(&db)
    .await
    .expect("insert worker session");

    let roster = manager
        .describe_run_members(&run.id)
        .await
        .expect("describe run members");

    assert_eq!(roster.team_id, team.id);
    assert_eq!(roster.run_id, run.id);
    assert_eq!(roster.members.len(), 2);

    let leader = &roster.members[0];
    assert_eq!(leader.member_id, "leader");
    assert_eq!(leader.display_name, "Leader Agent");
    assert_eq!(leader.role, "leader");
    assert_eq!(leader.description.as_deref(), Some("Lead planner"));
    assert_eq!(leader.agent_status.as_deref(), Some("running"));
    assert_eq!(leader.session_id.as_deref(), Some("session-leader"));
    assert_eq!(leader.session_status.as_deref(), Some("running"));
    assert_eq!(leader.card.description, "Lead planner");
    assert_eq!(leader.steps.len(), 1);
    assert_eq!(leader.steps[0].step_id, leader_step.id);
    assert_eq!(leader.steps[0].status, TeamStepStatus::Working);
    assert_eq!(
        leader.steps[0].session_id.as_deref(),
        Some("session-leader")
    );
    assert_eq!(leader.steps[0].session_status.as_deref(), Some("running"));

    let worker = &roster.members[1];
    assert_eq!(worker.member_id, "worker");
    assert_eq!(worker.display_name, "Worker Agent");
    assert_eq!(worker.role, "worker");
    assert_eq!(worker.description.as_deref(), Some("Implements changes"));
    assert_eq!(worker.agent_status.as_deref(), Some("idle"));
    assert_eq!(worker.session_id.as_deref(), Some("session-worker"));
    assert_eq!(worker.session_status.as_deref(), Some("running"));
    assert_eq!(worker.steps.len(), 1);
    assert_eq!(worker.steps[0].step_id, worker_step.id);
    assert_eq!(worker.steps[0].status, TeamStepStatus::Submitted);
    assert!(worker.steps[0].session_id.is_none());
    assert!(worker.steps[0].session_status.is_none());
}

#[tokio::test]
async fn describe_team_runtime_returns_member_runtime_status() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "describe-team-runtime".to_string(),
            description: Some("team to verify runtime status".to_string()),
            spec: json!({
                "entrypoint":"leader",
                "members":[
                    {"member_id":"leader","role":"leader","description":"Lead planner"},
                    {"member_id":"worker","role":"worker","description":"Implements changes"}
                ]
            }),
        })
        .await
        .expect("create team");

    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind("leader")
    .bind("Leader Agent")
    .bind("/tmp/leader")
    .bind("codex")
    .bind("[]")
    .bind("use_existing")
    .bind("manual")
    .bind("running")
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert leader agent");

    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind("worker")
    .bind("Worker Agent")
    .bind("/tmp/worker")
    .bind("codex")
    .bind("[]")
    .bind("create_worktree")
    .bind("manual")
    .bind("idle")
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert worker agent");

    sqlx::query(
        r#"
        INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
        VALUES (?1, ?2, ?3, ?4, NULL)
        "#,
    )
    .bind("session-leader")
    .bind("leader")
    .bind("running")
    .bind(10_i64)
    .execute(&db)
    .await
    .expect("insert leader session");

    let runtime = manager
        .describe_team_runtime(&team.id)
        .await
        .expect("describe team runtime");

    assert_eq!(runtime.team_id, team.id);
    assert_eq!(runtime.team_name, team.name);
    assert_eq!(runtime.status, crate::team::TeamRuntimeStatus::Degraded);
    assert_eq!(runtime.members.len(), 2);

    let leader = &runtime.members[0];
    assert_eq!(leader.member_id, "leader");
    assert_eq!(leader.display_name, "Leader Agent");
    assert_eq!(leader.session_id.as_deref(), Some("session-leader"));
    assert_eq!(leader.session_status.as_deref(), Some("running"));
    assert_eq!(leader.card.description, "Lead planner");

    let worker = &runtime.members[1];
    assert_eq!(worker.member_id, "worker");
    assert_eq!(worker.display_name, "Worker Agent");
    assert!(worker.session_id.is_none());
    assert!(worker.session_status.is_none());
    assert_eq!(worker.card.description, "Implements changes");
}

#[tokio::test]
async fn describe_team_context_merges_runtime_summary_and_optional_run_overlay() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "describe-team-context".to_string(),
            description: Some("team to verify merged context view".to_string()),
            spec: json!({
                "entrypoint":"leader",
                "members":[
                    {"member_id":"leader","role":"leader","description":"Lead planner"},
                    {"member_id":"worker","role":"worker","description":"Implements changes"}
                ]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-team-context"), json!({"prompt":"go"}))
        .await
        .expect("create run");
    let leader_step = manager
        .submit_step(&run.id, "leader_plan", "leader", Vec::new(), None)
        .await
        .expect("submit leader step");

    sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, source, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind("leader")
    .bind("Leader Agent")
    .bind("/tmp/leader")
    .bind("codex")
    .bind("[]")
    .bind("use_existing")
    .bind("manual")
    .bind("running")
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert leader agent");

    sqlx::query(
        r#"
        INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
        VALUES (?1, ?2, ?3, ?4, NULL)
        "#,
    )
    .bind("session-leader")
    .bind("leader")
    .bind("running")
    .bind(10_i64)
    .execute(&db)
    .await
    .expect("insert leader session");

    manager
        .start_step(&leader_step.id, Some("session-leader"))
        .await
        .expect("start leader step");

    manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "leader",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "worker",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!("## Review request\n\nPlease inspect the patch."),
            idempotency_key: Some("ctx-unread-worker"),
        })
        .await
        .expect("send unread worker message");

    let team_context = manager
        .describe_team_context(Some(&team.id), Some(&run.id))
        .await
        .expect("describe team context");

    assert_eq!(team_context.team_id, team.id);
    assert_eq!(
        team_context.runtime.status,
        crate::team::TeamRuntimeStatus::Degraded
    );
    assert_eq!(team_context.runtime.online_count, 1);
    assert_eq!(team_context.runtime.member_count, 2);
    assert_eq!(
        team_context
            .run
            .as_ref()
            .map(|overlay| overlay.run_id.as_str()),
        Some(run.id.as_str())
    );
    assert_eq!(team_context.members.len(), 2);
    assert_eq!(team_context.members[0].display_name, "Leader Agent");
    assert_eq!(team_context.members[0].pending_inbox_count, 0);
    assert_eq!(team_context.members[0].steps.len(), 1);
    assert_eq!(team_context.members[1].pending_inbox_count, 1);

    let runtime_only_context = manager
        .describe_team_context(Some(&team.id), None)
        .await
        .expect("describe runtime-only team context");
    assert_eq!(runtime_only_context.team_id, team.id);
    assert_eq!(
        runtime_only_context.runtime.status,
        crate::team::TeamRuntimeStatus::Degraded
    );
    assert!(runtime_only_context.run.is_none());
    assert_eq!(runtime_only_context.members.len(), 2);
    assert_eq!(runtime_only_context.members[0].pending_inbox_count, 0);
    assert_eq!(runtime_only_context.members[1].pending_inbox_count, 0);
    assert!(runtime_only_context.members[0].steps.is_empty());
}
