use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod archive_cases;
mod channel_cases;
mod run_admin_cases;
mod runtime_view_cases;
mod task_cases;

use super::codec::team_run_status_from_str;
use super::{TeamManager, message_archive_body_text, task_conversation_payload_correlation_id};
use crate::acp::{AcpActorSkillContext, DEFAULT_ACTOR_CHANNEL};
use crate::agent::{WorktreeMode, derive_team_runtime_workdir};
use crate::internal::client::InternalGrpcPeerClientConfig;
use crate::internal::tls::InternalGrpcSecurityMode;
use crate::team::{
    SendActorMessageInput, TeamActorMessageStatus, TeamActorMessageTransport, TeamDefinitionConfig,
    TeamRunEventRecord, TeamRunResumeError, TeamRunStatus, TeamStepStatus,
    TeamTaskAssignmentUpdate, TeamTaskContextPatch, TeamTaskListQuery, TeamTaskStatus,
};
use agenthub_db::AgentEventDbRouter;
use agenthub_message_archive::{
    MessageArchiveStore, MessageDocument, MessageDocumentKind, MessageSearchHit, MessageSearchQuery,
};
use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ACTOR_NODE_PEER_ID, ActorAckRequest, ActorIdentityKind, ActorInboxRequest,
    ActorMailboxService, ActorMessageHandlingDisposition, ActorMessageTaskRelation,
    ActorSendRequest, ActorServiceErrorCode, ActorTaskLinkRequest, ActorTriageRequest,
};
use async_trait::async_trait;
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
use tokio::time::{sleep, timeout};
use uuid::Uuid;

#[derive(Default)]
struct RecordingMessageArchive {
    documents: Mutex<Vec<MessageDocument>>,
}

#[async_trait]
impl MessageArchiveStore for RecordingMessageArchive {
    async fn ensure_ready(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn append_documents(&self, documents: &[MessageDocument]) -> anyhow::Result<()> {
        self.documents.lock().await.extend_from_slice(documents);
        Ok(())
    }

    async fn search(&self, _query: &MessageSearchQuery) -> anyhow::Result<Vec<MessageSearchHit>> {
        Ok(Vec::new())
    }
}

struct TailAppendingMessageArchive {
    db: SqlitePool,
    conversation_id: String,
    task_id: String,
    run_id: Option<String>,
    inserted: Mutex<bool>,
    documents: Mutex<Vec<MessageDocument>>,
}

#[test]
fn task_conversation_payload_correlation_id_normalizes_optional_payload_field() {
    assert_eq!(
        task_conversation_payload_correlation_id(&json!({"correlation_id":" corr-1 "})),
        "corr-1"
    );
    assert_eq!(
        task_conversation_payload_correlation_id(&json!({"correlation_id":"   "})),
        ""
    );
    assert_eq!(task_conversation_payload_correlation_id(&json!({})), "");
    assert_eq!(
        task_conversation_payload_correlation_id(&json!({"correlation_id":42})),
        ""
    );
}

#[async_trait]
impl MessageArchiveStore for TailAppendingMessageArchive {
    async fn ensure_ready(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn append_documents(&self, documents: &[MessageDocument]) -> anyhow::Result<()> {
        self.documents.lock().await.extend_from_slice(documents);
        let should_insert = {
            let mut inserted = self.inserted.lock().await;
            if *inserted {
                false
            } else {
                *inserted = true;
                true
            }
        };
        if should_insert {
            sqlx::query(
                r#"
                INSERT INTO team_conversation_messages (
                    conversation_id,
                    task_id,
                    from_actor_id,
                    to_actor_id,
                    route,
                    payload_json,
                    created_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
            )
            .bind(&self.conversation_id)
            .bind(&self.task_id)
            .bind("user")
            .bind("coordinator")
            .bind("to_coordinator")
            .bind(json!({"type":"chat_message","text":"live tail message"}).to_string())
            .bind(Utc::now().timestamp())
            .execute(&self.db)
            .await?;
            if let Some(run_id) = self.run_id.as_deref() {
                sqlx::query(
                    r#"
                    INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                    VALUES (?1, NULL, ?2, ?3, ?4)
                    "#,
                )
                .bind(run_id)
                .bind("live_tail_event")
                .bind(Utc::now().timestamp())
                .bind(json!({"text":"live tail run event"}).to_string())
                .execute(&self.db)
                .await?;
                sqlx::query(
                    r#"
                    INSERT INTO team_actor_messages (
                        run_id,
                        from_actor_id,
                        from_peer_id,
                        to_actor_id,
                        to_peer_id,
                        channel,
                        transport,
                        route_json,
                        payload_json,
                        status,
                        created_at
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, ?9, ?10)
                    "#,
                )
                .bind(run_id)
                .bind("coordinator")
                .bind(ACTOR_MAIN_PEER_ID)
                .bind("worker-1")
                .bind(ACTOR_MAIN_PEER_ID)
                .bind("all")
                .bind("local")
                .bind(json!({"type":"chat_message","text":"live tail actor message"}).to_string())
                .bind("pending")
                .bind(Utc::now().timestamp())
                .execute(&self.db)
                .await?;
            }
        }
        Ok(())
    }

    async fn search(&self, _query: &MessageSearchQuery) -> anyhow::Result<Vec<MessageSearchHit>> {
        Ok(Vec::new())
    }
}

struct PendingMessageArchive;

#[async_trait]
impl MessageArchiveStore for PendingMessageArchive {
    async fn ensure_ready(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn append_documents(&self, _documents: &[MessageDocument]) -> anyhow::Result<()> {
        std::future::pending::<()>().await;
        Ok(())
    }

    async fn search(&self, _query: &MessageSearchQuery) -> anyhow::Result<Vec<MessageSearchHit>> {
        Ok(Vec::new())
    }
}

async fn wait_for_archive_documents(
    archive: &RecordingMessageArchive,
    expected_len: usize,
) -> Vec<MessageDocument> {
    timeout(Duration::from_secs(2), async {
        loop {
            let documents = archive.documents.lock().await.clone();
            if documents.len() >= expected_len {
                return documents;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("archive documents should be appended")
}

async fn wait_for_archive_run_event_documents(
    archive: &RecordingMessageArchive,
    run_id: &str,
    expected_len: usize,
) -> Vec<MessageDocument> {
    timeout(Duration::from_secs(2), async {
        loop {
            let documents = archive.documents.lock().await.clone();
            let run_event_documents = documents
                .into_iter()
                .filter(|document| {
                    document.source_kind == MessageDocumentKind::TeamRunEvent
                        && document.run_id.as_deref() == Some(run_id)
                })
                .collect::<Vec<_>>();
            if run_event_documents.len() >= expected_len {
                return run_event_documents;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("archive run event documents should be appended")
}

fn archived_run_event_types<'a>(
    documents: &[MessageDocument],
    events: &'a [TeamRunEventRecord],
) -> Vec<&'a str> {
    documents
        .iter()
        .filter_map(|document| {
            events
                .iter()
                .find(|event| document.source_id == event.event_id.to_string())
                .map(|event| event.event_type.as_str())
        })
        .collect()
}

async fn assert_archive_documents_stay_empty(archive: &RecordingMessageArchive) {
    timeout(Duration::from_secs(2), async {
        loop {
            let documents = archive.documents.lock().await.clone();
            assert!(
                documents.is_empty(),
                "archive should remain empty, got documents: {documents:?}"
            );
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect_err("archive should remain empty for the full assertion window");
}

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
            group_id TEXT,
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
            correlation_id TEXT NOT NULL DEFAULT '',
            group_id TEXT,
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
    .execute(&pool)
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
    .execute(&pool)
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
    .execute(&pool)
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

fn task_attempt_number(task: &crate::team::TeamTaskRecord) -> Option<i64> {
    task.context
        .get("execution")
        .and_then(|value| value.get("attempt_number"))
        .and_then(Value::as_i64)
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
            conversation_id, task_id, from_actor_id, to_actor_id, route, group_id, payload_json, idempotency_key, created_at
        )
        SELECT ?1, ?2, ?3, NULL, 'broadcast', group_id, ?4, NULL, ?5
        FROM team_tasks
        WHERE id = ?2
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
async fn create_team_task_and_run_persist_authority_group_id() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team_with_owner(
            TeamDefinitionConfig {
                name: "group-authority-team".to_string(),
                description: Some("team with owner-backed group boundary".to_string()),
                spec: json!({
                    "entrypoint":"coordinator",
                    "members":[{"member_id":"coordinator","role":"coordinator"}]
                }),
            },
            Some("user-group-authority"),
        )
        .await
        .expect("create team");
    let (task, _) = manager
        .create_task(
            &team.id,
            "Group scoped task",
            "user",
            json!({"summary":"group boundary check"}),
            "group_chat",
            Some("all"),
        )
        .await
        .expect("create task");
    let run = manager
        .create_run(&team.id, Some(&task.id), json!({"task_id":task.id}))
        .await
        .expect("create run");

    let row = sqlx::query(
        r#"
        SELECT
            td.group_id AS team_group_id,
            tt.group_id AS task_group_id,
            tr.group_id AS run_group_id
        FROM team_definitions AS td
        JOIN team_tasks AS tt ON tt.team_id = td.id
        JOIN team_runs AS tr ON tr.team_id = td.id
        WHERE td.id = ?1
          AND tt.id = ?2
          AND tr.id = ?3
        "#,
    )
    .bind(&team.id)
    .bind(&task.id)
    .bind(&run.id)
    .fetch_one(&db)
    .await
    .expect("read authority group ids");
    assert_eq!(
        row.get::<Option<String>, _>("team_group_id"),
        Some("user-group-authority".to_string())
    );
    assert_eq!(
        row.get::<Option<String>, _>("task_group_id"),
        Some("user-group-authority".to_string())
    );
    assert_eq!(
        row.get::<Option<String>, _>("run_group_id"),
        Some("user-group-authority".to_string())
    );
}

#[tokio::test]
async fn append_task_conversation_message_persists_authority_group_id() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team_with_owner(
            TeamDefinitionConfig {
                name: "message-group-authority-team".to_string(),
                description: Some("team with message group boundary".to_string()),
                spec: json!({
                    "entrypoint":"coordinator",
                    "members":[{"member_id":"coordinator","role":"coordinator"}]
                }),
            },
            Some("user-message-authority"),
        )
        .await
        .expect("create team");
    let (task, _) = manager
        .create_task(
            &team.id,
            "Message group task",
            "user",
            json!({"summary":"message group boundary check"}),
            "group_chat",
            Some("all"),
        )
        .await
        .expect("create task");

    let (message, created) = manager
        .append_task_conversation_message_with_created(
            &task.id,
            "user",
            None,
            "group_chat",
            json!({
                "type": "chat_message",
                "text": "message with group",
                "correlation_id": "corr-message-authority"
            }),
            Some("message-authority-group-1"),
        )
        .await
        .expect("append message");
    assert!(created);
    assert_eq!(message.group_id.as_deref(), Some("user-message-authority"));

    let stored_group_id: Option<String> =
        sqlx::query_scalar("SELECT group_id FROM team_conversation_messages WHERE id = ?1")
            .bind(message.message_id)
            .fetch_one(&db)
            .await
            .expect("read task message group_id");
    assert_eq!(stored_group_id, Some("user-message-authority".to_string()));
}

#[tokio::test]
async fn send_actor_message_persists_authority_group_id() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team_with_owner(
            TeamDefinitionConfig {
                name: "actor-message-group-authority-team".to_string(),
                description: Some("team with actor message group boundary".to_string()),
                spec: json!({
                    "entrypoint":"coordinator",
                    "members":[
                        {"member_id":"coordinator","role":"coordinator"},
                        {"member_id":"worker-1","role":"worker"}
                    ]
                }),
            },
            Some("user-actor-message-authority"),
        )
        .await
        .expect("create team");
    let (task, _) = manager
        .create_task(
            &team.id,
            "Actor message group task",
            "user",
            json!({"summary":"actor message group boundary check"}),
            "group_chat",
            Some("all"),
        )
        .await
        .expect("create task");
    let run = manager
        .create_run(&team.id, Some(&task.id), json!({"task_id":task.id}))
        .await
        .expect("create run");

    let message = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "coordinator",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "worker-1",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "all",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({
                "type": "chat_message",
                "text": "actor message with group"
            }),
            idempotency_key: Some("actor-message-authority-group-1"),
            message_kind: None,
        })
        .await
        .expect("send actor message");

    let stored_group_id: Option<String> =
        sqlx::query_scalar("SELECT group_id FROM team_actor_messages WHERE id = ?1")
            .bind(message.message_id)
            .fetch_one(&db)
            .await
            .expect("read actor message group_id");
    assert_eq!(
        stored_group_id,
        Some("user-actor-message-authority".to_string())
    );
}

#[tokio::test]
async fn task_and_conversation_messages_are_persisted_with_redaction() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-team".to_string(),
            description: Some("team for task persistence".to_string()),
            spec: json!({"entrypoint":"coordinator_plan","members":[{"member_id":"coordinator"}]}),
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
            "coordinator",
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
            spec: json!({"entrypoint":"coordinator_plan","members":[{"member_id":"coordinator"}]}),
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
            spec: json!({"entrypoint":"coordinator_plan","members":[{"member_id":"coordinator"}]}),
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
async fn append_task_conversation_message_persists_correlation_id_column() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-correlation-team".to_string(),
            description: Some("team for task message correlation".to_string()),
            spec: json!({"entrypoint":"coordinator_plan","members":[{"member_id":"coordinator"}]}),
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

    let (message, created) = manager
        .append_task_conversation_message_with_created(
            &task.id,
            "user",
            None,
            "group_chat",
            json!({"type":"chat_message","text":"hello team","correlation_id":"corr-task-authority-1"}),
            Some("task-corr-1"),
        )
        .await
        .expect("append message");
    assert!(created);

    let correlation_id: String =
        sqlx::query_scalar("SELECT correlation_id FROM team_conversation_messages WHERE id = ?1")
            .bind(message.message_id)
            .fetch_one(&db)
            .await
            .expect("read task message correlation_id");
    assert_eq!(correlation_id, "corr-task-authority-1");

    let direct_message = manager
        .append_task_conversation_message(
            &task.id,
            "user",
            Some("  "),
            "group_chat",
            json!({
                "type":"chat_message",
                "text":"hello without idempotency",
                "correlation_id":" corr-task-direct-1 "
            }),
        )
        .await
        .expect("append direct message");
    let direct_correlation_id: String =
        sqlx::query_scalar("SELECT correlation_id FROM team_conversation_messages WHERE id = ?1")
            .bind(direct_message.message_id)
            .fetch_one(&db)
            .await
            .expect("read direct task message correlation_id");
    assert_eq!(direct_correlation_id, "corr-task-direct-1");
}

#[tokio::test]
async fn run_context_read_models_reflect_actor_and_session_state() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "run-context-read-model-team".to_string(),
            description: Some("team for run context read models".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator"},
                    {"member_id":"worker","role":"worker"},
                    {"member_id":"reviewer","role":"reviewer"}
                ]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-read-models"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");
    manager
        .append_run_event(&run.id, "operator_note", json!({"text":"checkpoint"}))
        .await
        .expect("append run event");

    let pending = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "coordinator",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "worker",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "all",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({"type":"chat_message","text":"please take this"}),
            idempotency_key: Some("read-model-pending"),
            message_kind: None,
        })
        .await
        .expect("send pending actor message");
    let delivered = manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "coordinator",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "all",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({"type":"chat_message","text":"please review this"}),
            idempotency_key: Some("read-model-delivered"),
            message_kind: None,
        })
        .await
        .expect("send delivered actor message");
    manager
        .ack_actor_message(&run.id, "reviewer", delivered.message_id)
        .await
        .expect("ack reviewer message");

    sqlx::query("INSERT INTO agents (id, name, workdir, command, args, worktree_mode, code_mode, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)")
        .bind("worker")
        .bind("Worker")
        .bind("/tmp/worker")
        .bind("agent")
        .bind("[]")
        .bind("off")
        .bind(1_i64)
        .bind("running")
        .bind(10_i64)
        .bind(10_i64)
        .execute(&db)
        .await
        .expect("insert worker agent");
    sqlx::query("INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at) VALUES (?1, ?2, ?3, ?4, NULL)")
        .bind("session-worker-live")
        .bind("worker")
        .bind("running")
        .bind(10_i64)
        .execute(&db)
        .await
        .expect("insert live worker session");

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let latest_event_id = events
        .iter()
        .map(|event| event.event_id)
        .max()
        .expect("run should have events");

    let fingerprint = manager
        .read_run_context_fingerprint(&run.id)
        .await
        .expect("read run fingerprint");
    assert_eq!(fingerprint.team_id, team.id);
    assert_eq!(fingerprint.run_id, run.id);
    assert_eq!(fingerprint.run_status, "submitted");
    assert_eq!(fingerprint.latest_event_id, latest_event_id);
    assert_eq!(fingerprint.latest_mailbox_message_id, delivered.message_id);
    assert_eq!(fingerprint.mailbox_pending, 1);
    assert_eq!(fingerprint.mailbox_delivered, 1);
    assert_eq!(fingerprint.mailbox_dead_letter, 0);

    let pending_by_actor = manager
        .list_actor_pending_counts_by_actor(&run.id)
        .await
        .expect("list pending counts by actor");
    assert_eq!(pending_by_actor.get("worker"), Some(&1));
    assert_eq!(pending_by_actor.get("reviewer"), None);

    let all_pending = manager
        .list_pending_actor_unread_counts()
        .await
        .expect("list pending unread counts");
    assert!(all_pending.iter().any(|record| {
        record.run_id == run.id && record.actor_id == "worker" && record.unread_count == 1
    }));
    assert!(
        !all_pending
            .iter()
            .any(|record| { record.run_id == run.id && record.actor_id == "reviewer" })
    );

    assert_eq!(
        manager
            .member_role_for_run(&run.id, "worker")
            .await
            .expect("read worker role"),
        Some("worker".to_string())
    );
    assert_eq!(
        manager
            .member_role_for_run(&run.id, "missing")
            .await
            .expect("read missing role"),
        None
    );
    assert_eq!(
        manager
            .member_role_for_run("missing-run", "worker")
            .await
            .expect("read missing run role"),
        None
    );

    assert_eq!(
        manager
            .get_agent_session_status("session-worker-live")
            .await
            .expect("read session status"),
        Some("running".to_string())
    );
    assert_eq!(
        manager
            .get_agent_session_status("missing-session")
            .await
            .expect("read missing session status"),
        None
    );
    assert_eq!(
        manager
            .get_live_member_session("worker")
            .await
            .expect("read live worker session"),
        Some(("session-worker-live".to_string(), "running".to_string()))
    );
    assert_eq!(
        manager
            .get_live_member_session("missing-worker")
            .await
            .expect("read missing live session"),
        None
    );

    let mismatch = manager
        .describe_team_context(Some("wrong-team"), Some(&run.id))
        .await
        .expect_err("explicit team mismatch should be rejected");
    assert!(mismatch.to_string().contains("wrong-team"));
    assert_eq!(pending.status, TeamActorMessageStatus::Pending);
}

async fn migrate_team_messages_to_archive_replays_agent_events_with_acp_aggregation() {
    let db = setup_test_db().await;
    let event_dbs = AgentEventDbRouter::new(std::env::temp_dir().join(format!(
        "agenthub-archive-acp-eventdb-{}",
        uuid::Uuid::new_v4()
    )));

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
    .bind(std::env::temp_dir().to_string_lossy().to_string())
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
    sqlx::query(
        r#"
        INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
        VALUES (?1, ?2, ?3, ?4, NULL)
        "#,
    )
    .bind("main-session")
    .bind("planner")
    .bind("running")
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert main session");
    sqlx::query(
        r#"
        INSERT INTO agent_events (agent_id, session_id, seq, ts, stream, message)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind("planner")
    .bind("main-session")
    .bind("1")
    .bind(10_i64)
    .bind("acp")
    .bind(r#"{"type":"tool_call","text":"main raw event","api_key":"main-secret"}"#)
    .execute(&db)
    .await
    .expect("insert main agent event");

    sqlx::query(
        r#"
        INSERT INTO team_definitions (id, name, description, spec_json, owner_user_id, created_at, updated_at)
        VALUES (?1, ?2, NULL, ?3, NULL, ?4, ?5)
        "#,
    )
    .bind("archive-team")
    .bind("archive-team")
    .bind(json!({"entrypoint":"coordinator_plan","members":[{"member_id":"planner"}]}).to_string())
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert archive team");
    sqlx::query(
        r#"
        INSERT INTO team_tasks (id, team_id, title, status, priority, created_by_actor_id, assigned_member_id, context_json, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, 'medium', ?5, NULL, ?6, ?7, ?8)
        "#,
    )
    .bind("archive-task")
    .bind("archive-team")
    .bind("Archive ACP")
    .bind("open")
    .bind("user")
    .bind(json!({}).to_string())
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert archive task");
    sqlx::query(
        r#"
        INSERT INTO team_conversations (id, team_id, task_id, mode, topic, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind("archive-conversation")
    .bind("archive-team")
    .bind("archive-task")
    .bind("group_chat")
    .bind("Archive ACP")
    .bind(1_i64)
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert archive conversation");
    sqlx::query(
        r#"
        INSERT INTO team_runs (id, team_id, context_id, status, input_json, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind("archive-run")
    .bind("archive-team")
    .bind("archive-task")
    .bind("working")
    .bind(json!({"task_id":"archive-task","conversation_id":"archive-conversation"}).to_string())
    .bind(1_i64)
    .execute(&db)
    .await
    .expect("insert archive run");
    sqlx::query(
        r#"
        INSERT INTO team_steps (
            id, run_id, step_key, member_id, remote_task_id, status, attempt, depends_on_json, input_json, started_at, ended_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, NULL, ?8, ?9)
        "#,
    )
    .bind("archive-step")
    .bind("archive-run")
    .bind("planner_step")
    .bind("planner")
    .bind("per-agent-session")
    .bind("working")
    .bind("[]")
    .bind(1_i64)
    .bind(22_i64)
    .execute(&db)
    .await
    .expect("insert archive step");
    sqlx::query(
        r#"
        INSERT INTO team_runs (id, team_id, context_id, status, input_json, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind("archive-shared-mailbox-run")
    .bind("archive-team")
    .bind("archive-task")
    .bind("working")
    .bind(
        json!({
            "bootstrap_kind": "shared_thread_mailbox",
            "task_id": "archive-task",
            "conversation_id": "archive-conversation"
        })
        .to_string(),
    )
    .bind(23_i64)
    .execute(&db)
    .await
    .expect("insert shared mailbox run");
    sqlx::query(
        r#"
        INSERT INTO team_steps (
            id, run_id, step_key, member_id, remote_task_id, status, attempt, depends_on_json, input_json, started_at, ended_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, NULL, ?8, ?9)
        "#,
    )
    .bind("archive-shared-mailbox-step")
    .bind("archive-shared-mailbox-run")
    .bind("planner_step")
    .bind("planner")
    .bind("per-agent-session")
    .bind("working")
    .bind("[]")
    .bind(23_i64)
    .bind(25_i64)
    .execute(&db)
    .await
    .expect("insert shared mailbox step");
    sqlx::query(
        r#"
        INSERT INTO team_tasks (id, team_id, title, status, priority, created_by_actor_id, assigned_member_id, context_json, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, 'medium', ?5, NULL, ?6, ?7, ?8)
        "#,
    )
    .bind("archive-task-2")
    .bind("archive-team")
    .bind("Archive ACP 2")
    .bind("open")
    .bind("user")
    .bind(json!({}).to_string())
    .bind(30_i64)
    .bind(30_i64)
    .execute(&db)
    .await
    .expect("insert second archive task");
    sqlx::query(
        r#"
        INSERT INTO team_conversations (id, team_id, task_id, mode, topic, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind("archive-conversation-2")
    .bind("archive-team")
    .bind("archive-task-2")
    .bind("group_chat")
    .bind("Archive ACP 2")
    .bind(30_i64)
    .bind(30_i64)
    .execute(&db)
    .await
    .expect("insert second archive conversation");
    sqlx::query(
        r#"
        INSERT INTO team_runs (id, team_id, context_id, status, input_json, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind("archive-run-2")
    .bind("archive-team")
    .bind("archive-task-2")
    .bind("working")
    .bind(
        json!({"task_id":"archive-task-2","conversation_id":"archive-conversation-2"}).to_string(),
    )
    .bind(30_i64)
    .execute(&db)
    .await
    .expect("insert second archive run");
    sqlx::query(
        r#"
        INSERT INTO team_steps (
            id, run_id, step_key, member_id, remote_task_id, status, attempt, depends_on_json, input_json, started_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, NULL, ?8)
        "#,
    )
    .bind("archive-step-2")
    .bind("archive-run-2")
    .bind("planner_step")
    .bind("planner")
    .bind("per-agent-session")
    .bind("working")
    .bind("[]")
    .bind(30_i64)
    .execute(&db)
    .await
    .expect("insert second archive step");

    let event_db = event_dbs
        .pool_for_agent("planner")
        .await
        .expect("open planner event db");
    for (seq, ts, stream, message) in [
        (
            "1",
            20_i64,
            "acp",
            r#"{"type":"agent_message","text":"lo","chunk":true,"message_id":"msg-1","chunk_index":1}"#,
        ),
        (
            "2",
            21_i64,
            "acp",
            r#"{"type":"agent_message","text":"hel","chunk":true,"message_id":"msg-1","chunk_index":0}"#,
        ),
        (
            "3",
            22_i64,
            "acp",
            r#"{"type":"agent_message","text":"fallback malformed chunk","chunk":true,"chunk_index":2,"api_key":"per-agent-secret"}"#,
        ),
        (
            "4",
            23_i64,
            "stdout",
            r#"{"type":"agent_message","text":"stdout raw chunk","chunk":true,"message_id":"stdout-msg","chunk_index":0}"#,
        ),
        (
            "5",
            24_i64,
            "acp",
            r#"{"type":"tool_call","text":"hidden mailbox event"}"#,
        ),
        (
            "6",
            31_i64,
            "acp",
            r#"{"type":"tool_call","text":"second run event"}"#,
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO agent_events (session_id, seq, ts, stream, message)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind("per-agent-session")
        .bind(seq)
        .bind(ts)
        .bind(stream)
        .bind(message)
        .execute(&event_db)
        .await
        .expect("insert per-agent event");
    }

    let archive = Arc::new(RecordingMessageArchive::default());
    let archive_manager =
        TeamManager::new_with_event_dbs_and_message_archive(db, event_dbs, Some(archive.clone()));
    let report = archive_manager
        .migrate_team_messages_to_archive(2)
        .await
        .expect("migrate agent events");

    assert_eq!(report.agent_events, 5);
    assert_eq!(report.aggregated_acp_messages, 1);
    assert_eq!(report.total_documents(), 6);

    let documents = archive.documents.lock().await.clone();
    assert_eq!(documents.len(), 6);
    assert!(documents.iter().any(|document| {
        document.source_kind == MessageDocumentKind::AgentEvent
            && document.document_id == "agent_event:planner:main-session:1"
            && document.body_text == "main raw event"
            && document.payload_json.as_deref().is_some_and(|payload| {
                payload.contains("[redacted]") && !payload.contains("main-secret")
            })
    }));
    assert!(documents.iter().any(|document| {
        document.source_kind == MessageDocumentKind::AgentEvent
            && document.document_id == "agent_event:planner:per-agent-session:3"
            && document.team_id.as_deref() == Some("archive-team")
            && document.run_id.as_deref() == Some("archive-run")
            && document.conversation_id.as_deref() == Some("archive-conversation")
            && document.task_id.as_deref() == Some("archive-task")
            && document.body_text == "fallback malformed chunk"
            && document.payload_json.as_deref().is_some_and(|payload| {
                payload.contains("[redacted]") && !payload.contains("per-agent-secret")
            })
    }));
    assert!(documents.iter().any(|document| {
        document.source_kind == MessageDocumentKind::AgentEvent
            && document.document_id == "agent_event:planner:per-agent-session:4"
            && document.logical_message_id.is_none()
            && document.team_id.is_none()
            && document.body_text == "stdout raw chunk"
    }));
    assert!(documents.iter().any(|document| {
        document.source_kind == MessageDocumentKind::AgentEvent
            && document.document_id == "agent_event:planner:per-agent-session:5"
            && document.team_id.is_none()
            && document.body_text == "hidden mailbox event"
    }));
    assert!(documents.iter().any(|document| {
        document.source_kind == MessageDocumentKind::AgentEvent
            && document.document_id == "agent_event:planner:per-agent-session:6"
            && document.team_id.as_deref() == Some("archive-team")
            && document.run_id.as_deref() == Some("archive-run-2")
            && document.conversation_id.as_deref() == Some("archive-conversation-2")
            && document.task_id.as_deref() == Some("archive-task-2")
            && document.body_text == "second run event"
    }));
    assert!(documents.iter().any(|document| {
        document.source_kind == MessageDocumentKind::AggregatedAcpMessage
            && document.document_id
                == "aggregated_acp_message:planner:per-agent-session:msg-1:agent_message"
            && document.source_id == "planner:per-agent-session:msg-1:agent_message"
            && document.logical_message_id.as_deref() == Some("msg-1")
            && document.team_id.as_deref() == Some("archive-team")
            && document.run_id.as_deref() == Some("archive-run")
            && document.conversation_id.as_deref() == Some("archive-conversation")
            && document.task_id.as_deref() == Some("archive-task")
            && document.agent_id.as_deref() == Some("planner")
            && document.session_id.as_deref() == Some("per-agent-session")
            && document.body_text == "hello"
            && document.event_id_from == Some(1)
            && document.event_id_to == Some(2)
            && document.chunk_count == Some(2)
    }));
}

#[tokio::test]
async fn append_task_conversation_message_propagates_non_idempotency_insert_failures() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-idempotency-insert-failure-team".to_string(),
            description: Some("team for task message insert failure".to_string()),
            spec: json!({"entrypoint":"coordinator_plan","members":[{"member_id":"coordinator"}]}),
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
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "input-step-template-run-team".to_string(),
            description: Some("team with run input step template".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator"},
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
                        "step_key":"coordinator-plan",
                        "member_id":"coordinator",
                        "execution":{"mode":"single_pass"}
                    },
                    {
                        "step_key":"worker-implement",
                        "member_id":"worker-1",
                        "depends_on":["coordinator-plan"],
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
    assert_eq!(steps[0].step_key, "coordinator-plan");
    assert_eq!(steps[0].member_id, "coordinator");
    assert!(steps[0].depends_on.is_empty());
    assert_eq!(steps[0].input, None);
    assert_eq!(steps[1].step_key, "worker-implement");
    assert_eq!(steps[1].member_id, "worker-1");
    assert_eq!(steps[1].depends_on, vec!["coordinator-plan".to_string()]);
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

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let documents = wait_for_archive_run_event_documents(&archive, &run.id, events.len()).await;
    let archived_event_types = archived_run_event_types(&documents, &events);
    assert!(
        archived_event_types.contains(&"run_submitted"),
        "run_submitted should be archived after run creation commits"
    );
    assert_eq!(
        archived_event_types
            .iter()
            .filter(|event_type| **event_type == "step_submitted")
            .count(),
        2,
        "materialized step_submitted events should be archived after run creation commits"
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
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator"},
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
            "coordinator",
            json!({
                "execution_plan": {
                    "steps": [
                        {
                            "step_key":"coordinator-plan",
                            "member_id":"coordinator",
                            "execution":{"mode":"single_pass"}
                        },
                        {
                            "step_key":"worker-implement",
                            "member_id":"worker-1",
                            "depends_on":["coordinator-plan"],
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
    assert_eq!(steps[0].step_key, "coordinator-plan");
    assert_eq!(steps[1].step_key, "worker-implement");
    assert_eq!(steps[1].depends_on, vec!["coordinator-plan".to_string()]);
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
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator"},
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
            spec: json!({"entrypoint":"coordinator","members":[{"member_id":"coordinator","role":"coordinator"}]}),
        })
        .await
        .expect("create team a");
    let team_b = manager
        .create_team(TeamDefinitionConfig {
            name: "run-team-b".to_string(),
            description: Some("foreign team".to_string()),
            spec: json!({"entrypoint":"coordinator","members":[{"member_id":"coordinator","role":"coordinator"}]}),
        })
        .await
        .expect("create team b");

    let (foreign_task, _) = manager
        .create_task(
            &team_b.id,
            "Foreign task",
            "coordinator",
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
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator"},
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
            "coordinator",
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
            "coordinator",
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
async fn list_tasks_with_query_keeps_tasks_without_conversation_rows() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "task-query-left-join".to_string(),
            description: Some("list tasks should not require conversation rows".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator"},
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
            "coordinator",
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
            "coordinator",
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
    assert_eq!(task_attempt_number(&after_start), Some(1));

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
    assert_eq!(task_attempt_number(&after_input_required), Some(1));

    let _ = manager
        .resume_step(&step.id, Some(json!({"answer":"approved"})))
        .await
        .expect("resume step");
    let after_resume = manager
        .get_task(&task.id)
        .await
        .expect("reload after resume");
    assert_eq!(after_resume.status, TeamTaskStatus::InProgress);
    assert_eq!(task_attempt_number(&after_resume), Some(2));

    let _ = manager
        .complete_step(&step.id, Some(json!({"result":"done"})))
        .await
        .expect("complete step");
    let after_complete = manager
        .get_task(&task.id)
        .await
        .expect("reload after complete");
    assert_eq!(after_complete.status, TeamTaskStatus::InReview);
    assert_eq!(task_attempt_number(&after_complete), Some(2));
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
    assert_eq!(task_attempt_number(&reloaded), None);
}

#[tokio::test]
async fn linked_run_create_sets_first_attempt_number() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "linked-task-attempt-create-team".to_string(),
            description: Some("team with linked task attempt projection".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let (task, _) = manager
        .create_task(
            &team.id,
            "Attempt-number task",
            "user",
            json!({"source":"ui"}),
            "group_chat",
            Some("attempt-create"),
        )
        .await
        .expect("create task");
    assert_eq!(task_attempt_number(&task), None);

    let _ = manager
        .create_run(
            &team.id,
            Some(&task.id),
            json!({"task_id": task.id, "prompt":"start linked execution"}),
        )
        .await
        .expect("create linked run");

    let reloaded = manager.get_task(&task.id).await.expect("reload task");
    assert_eq!(reloaded.status, TeamTaskStatus::InProgress);
    assert_eq!(task_attempt_number(&reloaded), Some(1));
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
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

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

    let documents = wait_for_archive_run_event_documents(&archive, &run.id, events.len()).await;
    let archived_event_types = archived_run_event_types(&documents, &events);
    assert!(
        archived_event_types.contains(&"step_canceled"),
        "step_canceled should be archived after transaction commit"
    );
    assert!(
        archived_event_types.contains(&"run_canceled"),
        "run_canceled should be archived after transaction commit"
    );
}

#[tokio::test]
async fn step_lifecycle_transitions_persist_and_emit_events() {
    let db = setup_test_db().await;
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

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

    let documents =
        wait_for_archive_run_event_documents(&archive, &run.id, event_types.len()).await;
    let mut archived_event_types = archived_run_event_types(&documents, &events);
    let mut expected_event_types = event_types.clone();
    archived_event_types.sort_unstable();
    expected_event_types.sort_unstable();
    assert_eq!(archived_event_types, expected_event_types);
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
async fn complete_step_offloads_large_output_to_coordinator_runtime_workspace_context_artifact() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!(
        "agenthub-coordinator-context-artifact-{unique_suffix}"
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
            name: "coordinator-artifact-team".to_string(),
            description: Some("team with coordinator continuity output".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner","role":"coordinator"}]
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
            member_role: Some("coordinator".to_string()),
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
        "artifact path should be under derived coordinator runtime workspace: {artifact_path}"
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
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        event_dbs.clone(),
        Some(archive.clone()),
    );

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

    let documents = wait_for_archive_run_event_documents(&archive, &run.id, events.len()).await;
    let archived_event_types = archived_run_event_types(&documents, &events);
    assert!(
        archived_event_types.contains(&"run_submitted"),
        "run_submitted should still be archived"
    );
    assert!(
        archived_event_types.contains(&"memory_flush_started"),
        "memory_flush_started should be archived after transaction commit"
    );
    assert_eq!(
        archived_event_types
            .iter()
            .filter(|event_type| **event_type == "memory_flush_started")
            .count(),
        2,
        "each flush attempt should archive its own memory_flush_started event"
    );
    assert!(
        archived_event_types.contains(&"memory_flush_persisted"),
        "memory_flush_persisted should be archived after transaction commit"
    );
    assert!(
        archived_event_types.contains(&"memory_flush_noop"),
        "memory_flush_noop should be archived after transaction commit"
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn flush_run_context_fails_when_session_mapping_missing() {
    let db = setup_test_db().await;
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

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

    let documents = wait_for_archive_documents(&archive, 3).await;
    let archived_event_types = documents
        .iter()
        .filter(|document| {
            document.source_kind == MessageDocumentKind::TeamRunEvent
                && document.run_id.as_deref() == Some(run.id.as_str())
        })
        .map(|document| document.body_text.as_str())
        .collect::<Vec<_>>();
    assert!(
        archived_event_types.contains(&"memory_flush_started"),
        "memory_flush_started should be archived for failed flush attempts"
    );
    assert!(
        archived_event_types.contains(&"memory_flush_failed"),
        "memory_flush_failed should be archived after transaction commit"
    );
}

#[tokio::test]
async fn input_required_and_resume_transitions_update_run_and_emit_events() {
    let db = setup_test_db().await;
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

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

    let documents =
        wait_for_archive_run_event_documents(&archive, &run.id, event_types.len()).await;
    let archived_event_types = archived_run_event_types(&documents, &events);
    assert!(
        archived_event_types.contains(&"run_input_required"),
        "run_input_required should be archived after transaction commit"
    );
    assert!(
        archived_event_types.contains(&"step_input_required"),
        "step_input_required should be archived after transaction commit"
    );
    assert!(
        archived_event_types.contains(&"step_resumed"),
        "step_resumed should be archived after transaction commit"
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
                    {"member_id":"planner","role":"coordinator"},
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
async fn continue_step_advances_reconcile_round_without_coordinator_resume() {
    let db = setup_test_db().await;
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "continue-reconcile-team".to_string(),
            description: Some("team for reconcile continue".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner","role":"coordinator"},
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

    let documents =
        wait_for_archive_run_event_documents(&archive, &run.id, event_types.len()).await;
    let archived_event_types = archived_run_event_types(&documents, &events);
    assert!(
        archived_event_types.contains(&"step_continued"),
        "step_continued should be archived after transaction commit"
    );
    assert_eq!(
        archived_event_types
            .iter()
            .filter(|event_type| **event_type == "step_reconcile_round_started")
            .count(),
        2,
        "both reconcile round start events should be archived"
    );
    assert!(
        archived_event_types.contains(&"step_reconcile_round_finished"),
        "step_reconcile_round_finished should be archived after transaction commit"
    );
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
                    {"member_id":"planner","role":"coordinator"},
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
                    {"member_id":"planner","role":"coordinator"},
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
                    {"member_id":"planner","role":"coordinator"},
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
            message_kind: None,
        })
        .await
        .expect("send message");
    let expected_payload = json!({
        "text":"please review",
        "source_kind":"agent",
        "source_surface":"mailbox",
        "requires_user_visible_reply":false
    });
    assert_eq!(sent.status, TeamActorMessageStatus::Pending);
    assert_eq!(sent.transport, TeamActorMessageTransport::Local);
    assert_eq!(sent.payload, expected_payload);
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
            message_kind: None,
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
async fn summarize_open_reply_obligations_prefers_lightweight_snapshot_loader() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db);

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "reply-obligation-team".to_string(),
            description: Some("team for reply obligation summary".to_string()),
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
            Some("ctx-reply-obligation"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "user:alice",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "reviewer",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: json!({
                "type":"chat_message",
                "text":"Need update",
                "requires_user_visible_reply":true
            }),
            idempotency_key: None,
            message_kind: None,
        })
        .await
        .expect("send inbound obligation");
    manager
        .send_actor_message(SendActorMessageInput {
            run_id: &run.id,
            from_actor_id: "reviewer",
            from_peer_id: ACTOR_MAIN_PEER_ID,
            to_actor_id: "user:alice",
            to_peer_id: ACTOR_MAIN_PEER_ID,
            channel: "coordination",
            transport: TeamActorMessageTransport::Local,
            route: None,
            payload: Value::String(
                r#"{"type":"chat_message","text":"Here is the update","correlation_id":"corr-summary"}"#
                    .to_string(),
            ),
            idempotency_key: None,
            message_kind: None,
        })
        .await
        .expect("send visible reply");

    let summary = manager
        .summarize_open_reply_obligations(&run.id)
        .await
        .expect("summarize reply obligations");

    assert_eq!(summary.open_total, 0);
    assert!(summary.open_by_actor.is_empty());
    assert!(summary.open_items.is_empty());
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
            message_kind: None,
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
            message_kind: None,
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
            message_kind: None,
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
            message_kind: None,
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
            message_kind: None,
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
            message_kind: None,
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
            message_kind: None,
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
async fn actor_mailbox_service_include_delivered_keeps_pending_visible_on_first_page() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-pending-first-team".to_string(),
            description: Some("team for delivered inbox pending-first behavior".to_string()),
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
            Some("ctx-msg-pending-first"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let mut latest_pending_id = None;
    for idx in 0..25 {
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
                payload: json!({"text": format!("message-{idx}")}),
                idempotency_key: Some(format!("msg-pending-first-{idx}")),
                message_kind: None,
            })
            .await
            .expect("actor send");
        latest_pending_id = Some(sent.message_id);
        if idx < 24 {
            service
                .actor_ack(ActorAckRequest {
                    run_id: run.id.clone(),
                    actor_id: "reviewer".to_string(),
                    message_id: sent.message_id,
                    ack_token: None,
                    result: None,
                })
                .await
                .expect("ack historical message");
        }
    }

    let inbox = service
        .actor_inbox(ActorInboxRequest {
            run_id: run.id,
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(20),
            states: Some(vec![
                TeamActorMessageStatus::Pending,
                TeamActorMessageStatus::Delivered,
            ]),
        })
        .await
        .expect("actor inbox with delivered keeps unread visible");

    assert_eq!(inbox.pending_count, 1);
    assert_eq!(inbox.messages.len(), 1);
    assert_eq!(inbox.messages[0].status, TeamActorMessageStatus::Pending);
    assert_eq!(inbox.messages[0].message_id, latest_pending_id.unwrap());
}

#[tokio::test]
async fn actor_mailbox_service_include_delivered_returns_history_when_unread_is_empty() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-history-only-team".to_string(),
            description: Some("team for delivered-only inbox history".to_string()),
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
            Some("ctx-msg-history-only"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let mut first_delivered_id = None;
    for idx in 0..3 {
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
                payload: json!({"text": format!("history-{idx}")}),
                idempotency_key: Some(format!("msg-history-only-{idx}")),
                message_kind: None,
            })
            .await
            .expect("actor send");
        if first_delivered_id.is_none() {
            first_delivered_id = Some(sent.message_id);
        }
        service
            .actor_ack(ActorAckRequest {
                run_id: run.id.clone(),
                actor_id: "reviewer".to_string(),
                message_id: sent.message_id,
                ack_token: None,
                result: None,
            })
            .await
            .expect("ack historical message");
    }

    let inbox = service
        .actor_inbox(ActorInboxRequest {
            run_id: run.id,
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(20),
            states: Some(vec![
                TeamActorMessageStatus::Pending,
                TeamActorMessageStatus::Delivered,
            ]),
        })
        .await
        .expect("actor inbox with delivered history only");

    assert_eq!(inbox.pending_count, 0);
    assert_eq!(inbox.messages.len(), 3);
    assert!(
        inbox
            .messages
            .iter()
            .all(|message| message.status == TeamActorMessageStatus::Delivered)
    );
    assert_eq!(inbox.messages[0].message_id, first_delivered_id.unwrap());
}

#[tokio::test]
async fn actor_mailbox_service_include_delivered_preserves_requested_mix_when_page_has_pending() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-mixed-first-page-team".to_string(),
            description: Some("team for delivered inbox mixed first page".to_string()),
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
            Some("ctx-msg-mixed-first-page"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    for idx in 0..3 {
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
                payload: json!({"text": format!("mixed-{idx}")}),
                idempotency_key: Some(format!("msg-mixed-first-page-{idx}")),
                message_kind: None,
            })
            .await
            .expect("actor send");
        if idx < 2 {
            service
                .actor_ack(ActorAckRequest {
                    run_id: run.id.clone(),
                    actor_id: "reviewer".to_string(),
                    message_id: sent.message_id,
                    ack_token: None,
                    result: None,
                })
                .await
                .expect("ack historical message");
        }
    }

    let inbox = service
        .actor_inbox(ActorInboxRequest {
            run_id: run.id,
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(20),
            states: Some(vec![
                TeamActorMessageStatus::Pending,
                TeamActorMessageStatus::Delivered,
            ]),
        })
        .await
        .expect("actor inbox with delivered mixed page");

    assert_eq!(inbox.pending_count, 1);
    assert_eq!(inbox.messages.len(), 3);
    assert_eq!(inbox.messages[0].status, TeamActorMessageStatus::Delivered);
    assert_eq!(inbox.messages[1].status, TeamActorMessageStatus::Delivered);
    assert_eq!(inbox.messages[2].status, TeamActorMessageStatus::Pending);
}

#[tokio::test]
async fn actor_mailbox_service_triage_hides_watching_messages_from_unread_snapshot() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-triage-team".to_string(),
            description: Some("team for mailbox triage unread semantics".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(&team.id, Some("ctx-msg-triage"), json!({"payload":"start"}))
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
            payload: json!({"text":"observe this request"}),
            idempotency_key: Some("msg-triage-watch-1".to_string()),
            message_kind: None,
        })
        .await
        .expect("actor send");

    let triaged = service
        .actor_triage(ActorTriageRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            message_id: sent.message_id,
            disposition: ActorMessageHandlingDisposition::Watching,
        })
        .await
        .expect("triage watching");
    assert_eq!(
        triaged.disposition,
        ActorMessageHandlingDisposition::Watching
    );
    assert!(triaged.handling_changed);
    assert_eq!(
        triaged.message.handled_by_actor_id.as_deref(),
        Some("reviewer")
    );

    let unread = service
        .actor_inbox(ActorInboxRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(20),
            states: None,
        })
        .await
        .expect("unread inbox after watch triage");
    assert_eq!(unread.pending_count, 0);
    assert!(unread.messages.is_empty());

    let with_history = service
        .actor_inbox(ActorInboxRequest {
            run_id: run.id,
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(20),
            states: Some(vec![
                TeamActorMessageStatus::Pending,
                TeamActorMessageStatus::Delivered,
            ]),
        })
        .await
        .expect("history inbox after watch triage");
    assert_eq!(with_history.messages.len(), 1);
    assert_eq!(
        with_history.messages[0].handling_disposition,
        ActorMessageHandlingDisposition::Watching
    );
}

#[tokio::test]
async fn actor_mailbox_service_claims_topics_and_prevents_parallel_takeover() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-claim-team".to_string(),
            description: Some("team for mailbox thread claim semantics".to_string()),
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
        .create_run(&team.id, Some("ctx-msg-claim"), json!({"payload":"start"}))
        .await
        .expect("create run");
    let task_title = "Review design".to_string();
    let (task, _) = manager
        .create_task_with_metadata(crate::team::TeamTaskCreateInput {
            team_id: &team.id,
            title: &task_title,
            created_by_actor_id: "planner",
            priority: crate::team::TeamTaskPriority::High,
            assigned_member_id: Some("reviewer"),
            context: json!({}),
            conversation_mode: "group_chat",
            topic: Some("mailbox claim"),
        })
        .await
        .expect("create task");

    let reviewer_message = service
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
                "text":"please take point on this topic",
                "task_id": task.id.clone(),
                "task_message_id": 77
            }),
            idempotency_key: Some("msg-claim-reviewer".to_string()),
            message_kind: None,
        })
        .await
        .expect("send reviewer message");
    let worker_message = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: Some("worker".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({
                "text":"same topic for another observer",
                "task_id": task.id.clone(),
                "task_message_id": 77
            }),
            idempotency_key: Some("msg-claim-worker".to_string()),
            message_kind: None,
        })
        .await
        .expect("send worker message");

    let reviewer_claim = service
        .actor_triage(ActorTriageRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            message_id: reviewer_message.message_id,
            disposition: ActorMessageHandlingDisposition::Claimed,
        })
        .await
        .expect("reviewer claim");
    assert_eq!(
        reviewer_claim.message.thread_owner_actor_id.as_deref(),
        Some("reviewer")
    );
    assert_eq!(
        reviewer_claim.message.thread_claim_status,
        Some(agenthub_team_actor::ActorThreadClaimStatus::Claimed)
    );

    let err = service
        .actor_triage(ActorTriageRequest {
            run_id: run.id.clone(),
            actor_id: "worker".to_string(),
            message_id: worker_message.message_id,
            disposition: ActorMessageHandlingDisposition::Claimed,
        })
        .await
        .expect_err("parallel takeover should conflict");
    assert_eq!(err.code, ActorServiceErrorCode::Conflict);
    assert!(err.message.contains("reviewer"));

    service
        .actor_triage(ActorTriageRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            message_id: reviewer_message.message_id,
            disposition: ActorMessageHandlingDisposition::Released,
        })
        .await
        .expect("release claim");

    let worker_claim = service
        .actor_triage(ActorTriageRequest {
            run_id: run.id,
            actor_id: "worker".to_string(),
            message_id: worker_message.message_id,
            disposition: ActorMessageHandlingDisposition::Claimed,
        })
        .await
        .expect("worker claim after release");
    assert_eq!(
        worker_claim.message.thread_owner_actor_id.as_deref(),
        Some("worker")
    );
}

#[tokio::test]
async fn actor_mailbox_service_requires_active_owner_for_release_and_complete() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-release-team".to_string(),
            description: Some("team for mailbox ownership guardrails".to_string()),
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
            Some("ctx-msg-release-guard"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let reviewer_message = service
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
                "text":"reviewer owns this topic",
                "correlation_id":"release-guard-1"
            }),
            idempotency_key: Some("msg-release-guard-reviewer".to_string()),
            message_kind: None,
        })
        .await
        .expect("send reviewer message");
    let worker_message = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: Some("worker".to_string()),
            channel_id: None,
            to_peer_id: None,
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Local),
            route: None,
            payload: json!({
                "text":"worker sees the same topic",
                "correlation_id":"release-guard-1"
            }),
            idempotency_key: Some("msg-release-guard-worker".to_string()),
            message_kind: None,
        })
        .await
        .expect("send worker message");

    service
        .actor_triage(ActorTriageRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            message_id: reviewer_message.message_id,
            disposition: ActorMessageHandlingDisposition::Claimed,
        })
        .await
        .expect("reviewer claim");

    let release_err = service
        .actor_triage(ActorTriageRequest {
            run_id: run.id.clone(),
            actor_id: "worker".to_string(),
            message_id: worker_message.message_id,
            disposition: ActorMessageHandlingDisposition::Released,
        })
        .await
        .expect_err("non-owner release should conflict");
    assert_eq!(release_err.code, ActorServiceErrorCode::Conflict);
    assert!(release_err.message.contains("reviewer"));

    let complete_err = service
        .actor_triage(ActorTriageRequest {
            run_id: run.id.clone(),
            actor_id: "worker".to_string(),
            message_id: worker_message.message_id,
            disposition: ActorMessageHandlingDisposition::Completed,
        })
        .await
        .expect_err("non-owner complete should conflict");
    assert_eq!(complete_err.code, ActorServiceErrorCode::Conflict);
    assert!(complete_err.message.contains("reviewer"));

    let worker_history = service
        .actor_inbox(ActorInboxRequest {
            run_id: run.id,
            actor_id: "worker".to_string(),
            cursor: None,
            limit: Some(20),
            states: Some(vec![TeamActorMessageStatus::Pending]),
        })
        .await
        .expect("worker history inbox");
    assert_eq!(worker_history.messages.len(), 1);
    assert_eq!(
        worker_history.messages[0].handling_disposition,
        ActorMessageHandlingDisposition::Untriaged
    );
    assert_eq!(
        worker_history.messages[0].thread_owner_actor_id.as_deref(),
        Some("reviewer")
    );
}

#[tokio::test]
async fn actor_mailbox_service_completed_claim_remains_visible_in_history() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-completed-team".to_string(),
            description: Some("team for completed topic visibility".to_string()),
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
            Some("ctx-msg-completed-visible"),
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
            payload: json!({
                "text":"finish this mailbox topic",
                "correlation_id":"completed-visible-1"
            }),
            idempotency_key: Some("msg-completed-visible-reviewer".to_string()),
            message_kind: None,
        })
        .await
        .expect("send mailbox message");

    service
        .actor_triage(ActorTriageRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            message_id: sent.message_id,
            disposition: ActorMessageHandlingDisposition::Claimed,
        })
        .await
        .expect("claim mailbox topic");
    let completed = service
        .actor_triage(ActorTriageRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            message_id: sent.message_id,
            disposition: ActorMessageHandlingDisposition::Completed,
        })
        .await
        .expect("complete mailbox topic");
    assert_eq!(
        completed.message.handling_disposition,
        ActorMessageHandlingDisposition::Completed
    );

    let unread = service
        .actor_inbox(ActorInboxRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(20),
            states: None,
        })
        .await
        .expect("unread inbox after completed triage");
    assert_eq!(unread.pending_count, 0);
    assert!(unread.messages.is_empty());

    let history = service
        .actor_inbox(ActorInboxRequest {
            run_id: run.id,
            actor_id: "reviewer".to_string(),
            cursor: None,
            limit: Some(20),
            states: Some(vec![
                TeamActorMessageStatus::Pending,
                TeamActorMessageStatus::Delivered,
            ]),
        })
        .await
        .expect("history inbox after completed triage");
    assert_eq!(history.messages.len(), 1);
    assert_eq!(
        history.messages[0].thread_claim_status,
        Some(agenthub_team_actor::ActorThreadClaimStatus::Completed)
    );
    assert_eq!(
        history.messages[0].thread_owner_actor_id.as_deref(),
        Some("reviewer")
    );
    assert_eq!(
        history.messages[0].handling_disposition,
        ActorMessageHandlingDisposition::Completed
    );
}

#[tokio::test]
async fn actor_mailbox_service_task_link_surfaces_durable_task_association() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-task-link-team".to_string(),
            description: Some("team for mailbox task link semantics".to_string()),
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
            Some("ctx-msg-task-link"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");
    let task_title = "Investigate mailbox".to_string();
    let (task, _) = manager
        .create_task_with_metadata(crate::team::TeamTaskCreateInput {
            team_id: &team.id,
            title: &task_title,
            created_by_actor_id: "planner",
            priority: crate::team::TeamTaskPriority::Medium,
            assigned_member_id: Some("reviewer"),
            context: json!({}),
            conversation_mode: "group_chat",
            topic: Some("task link"),
        })
        .await
        .expect("create task");

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
            payload: json!({"text":"convert this into a tracked lane"}),
            idempotency_key: Some("msg-task-link-reviewer".to_string()),
            message_kind: None,
        })
        .await
        .expect("send mailbox message");

    let linked = service
        .actor_task_link(ActorTaskLinkRequest {
            run_id: run.id.clone(),
            actor_id: "reviewer".to_string(),
            message_id: sent.message_id,
            task_id: task.id.clone(),
            relation: ActorMessageTaskRelation::SpawnedTask,
        })
        .await
        .expect("link mailbox message to task");
    assert!(linked.created);
    assert_eq!(linked.task_id, task.id);
    assert_eq!(linked.relation, ActorMessageTaskRelation::SpawnedTask);
    assert_eq!(
        linked.message.linked_task_id.as_deref(),
        Some(task.id.as_str())
    );
    assert_eq!(
        linked.message.linked_task_relation,
        Some(ActorMessageTaskRelation::SpawnedTask)
    );

    let link_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM team_actor_message_links WHERE run_id = ?1 AND message_id = ?2 AND task_id = ?3",
    )
    .bind(&run.id)
    .bind(sent.message_id)
    .bind(&task.id)
    .fetch_one(&db)
    .await
    .expect("count mailbox task links");
    assert_eq!(link_count, 1);
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
            message_kind: None,
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
            message_kind: None,
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
                message_kind: None,
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
async fn actor_mailbox_service_direct_remote_send_requires_relay_route_and_remote_peer() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let service = manager.actor_mailbox_service();

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "actor-mailbox-direct-remote-validation-team".to_string(),
            description: Some("team for direct remote mailbox validation".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "members":[
                    {"member_id":"planner"},
                    {"member_id":"reviewer"}
                ]
            }),
        })
        .await
        .expect("create team");
    let run = manager
        .create_run(
            &team.id,
            Some("ctx-direct-remote-validation"),
            json!({"payload":"start"}),
        )
        .await
        .expect("create run");

    let missing_route = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
            to_actor_id: Some("remote-reviewer".to_string()),
            channel_id: None,
            to_peer_id: Some(ACTOR_NODE_PEER_ID.to_string()),
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Remote),
            route: None,
            payload: json!({"text":"please review remotely"}),
            idempotency_key: Some("msg-direct-remote-missing-route".to_string()),
            message_kind: None,
        })
        .await
        .expect_err("remote direct send without route should fail");
    assert_eq!(missing_route.code, ActorServiceErrorCode::BadRequest);
    assert_eq!(
        missing_route.message,
        "route is required for remote transport"
    );

    let null_route = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
            to_actor_id: Some("remote-reviewer".to_string()),
            channel_id: None,
            to_peer_id: Some(ACTOR_NODE_PEER_ID.to_string()),
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Remote),
            route: Some(Value::Null),
            payload: json!({"text":"please review remotely"}),
            idempotency_key: Some("msg-direct-remote-null-route".to_string()),
            message_kind: None,
        })
        .await
        .expect_err("remote direct send with null route should fail");
    assert_eq!(null_route.code, ActorServiceErrorCode::BadRequest);
    assert_eq!(
        null_route.message,
        "route must be a JSON object for remote transport"
    );

    let empty_route = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
            to_actor_id: Some("remote-reviewer".to_string()),
            channel_id: None,
            to_peer_id: Some(ACTOR_NODE_PEER_ID.to_string()),
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Remote),
            route: Some(json!({})),
            payload: json!({"text":"please review remotely"}),
            idempotency_key: Some("msg-direct-remote-empty-route".to_string()),
            message_kind: None,
        })
        .await
        .expect_err("remote direct send with empty route should fail");
    assert_eq!(empty_route.code, ActorServiceErrorCode::BadRequest);
    assert_eq!(
        empty_route.message,
        "route must contain endpoint or grpc_target for remote transport"
    );

    let main_peer = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
            to_actor_id: Some("remote-reviewer".to_string()),
            channel_id: None,
            to_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Remote),
            route: Some(json!({"endpoint":"https://remote.example/mailbox"})),
            payload: json!({"text":"please review remotely"}),
            idempotency_key: Some("msg-direct-remote-main-peer".to_string()),
            message_kind: None,
        })
        .await
        .expect_err("remote direct send to main peer should fail");
    assert_eq!(main_peer.code, ActorServiceErrorCode::BadRequest);
    assert_eq!(
        main_peer.message,
        "to_peer_id must not be 'main' for remote transport"
    );

    let valid = service
        .actor_send(ActorSendRequest {
            run_id: run.id.clone(),
            from_actor_id: "planner".to_string(),
            from_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
            to_actor_id: Some("remote-reviewer".to_string()),
            channel_id: None,
            to_peer_id: Some(ACTOR_NODE_PEER_ID.to_string()),
            channel: Some("coordination".to_string()),
            transport: Some(TeamActorMessageTransport::Remote),
            route: Some(json!({"endpoint":"https://remote.example/mailbox"})),
            payload: json!({"text":"please review remotely"}),
            idempotency_key: Some("msg-direct-remote-valid".to_string()),
            message_kind: None,
        })
        .await
        .expect("valid remote direct send");
    assert_eq!(valid.state, TeamActorMessageStatus::Pending);
    assert_eq!(valid.message.to_peer_id, ACTOR_NODE_PEER_ID);
    assert_eq!(valid.message.transport, TeamActorMessageTransport::Remote);

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
    .expect("count persisted mailbox rows");
    assert_eq!(mailbox_count, 1);
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
            message_kind: None,
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
            message_kind: None,
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
            message_kind: None,
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
            message_kind: None,
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
            message_kind: None,
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
        message_kind: None,
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
            message_kind: None,
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
            message_kind: None,
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
            message_kind: None,
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
            message_kind: None,
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
            message_kind: None,
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
            message_kind: None,
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
            message_kind: None,
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
            message_kind: None,
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
            message_kind: None,
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
            message_kind: None,
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
            message_kind: None,
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
    let archive = Arc::new(RecordingMessageArchive::default());
    let manager = TeamManager::new_with_event_dbs_and_message_archive(
        db.clone(),
        AgentEventDbRouter::with_default_base_dir(),
        Some(archive.clone()),
    );

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

    let documents =
        wait_for_archive_run_event_documents(&archive, &run.id, event_types.len()).await;
    let archived_event_types = archived_run_event_types(&documents, &events);
    assert!(
        archived_event_types.contains(&"step_failed"),
        "step_failed should be archived after transaction commit"
    );
    assert!(
        archived_event_types.contains(&"run_failed"),
        "run_failed should be archived after transaction commit"
    );
}
