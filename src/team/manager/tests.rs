use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use super::codec::team_run_status_from_str;
use super::{TeamManager, TeamRunResumeError};
use crate::db::AgentEventDbRouter;
use crate::team::{
    SendActorMessageInput, TeamActorMessageStatus, TeamActorMessageTransport, TeamDefinitionConfig,
    TeamRunStatus, TeamStepStatus, TeamTaskStatus,
};
use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ACTOR_NODE_PEER_ID, ActorAckRequest, ActorIdentityKind, ActorInboxRequest,
    ActorMailboxService, ActorSendRequest, ActorServiceErrorCode,
};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::routing::any;
use axum::{Router, serve};
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use tokio::sync::Mutex;

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
        CREATE TABLE agent_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            agent_id TEXT NOT NULL,
            session_id TEXT NOT NULL,
            seq TEXT NOT NULL,
            ts INTEGER NOT NULL,
            stream TEXT NOT NULL,
            message BLOB NOT NULL,
            FOREIGN KEY(agent_id) REFERENCES agents(id)
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
    (format!("http://{addr}/relay"), captures, handle)
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

    let listed = manager.list_tasks(&team.id, 20).await.expect("list tasks");
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
    assert_eq!(working.remote_task_id.as_deref(), Some("remote-task-1"));
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

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let continuity_event = events
        .iter()
        .find(|event| event.event_type == "continuity_state_updated")
        .expect("continuity_state_updated event should exist");
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
    assert_eq!(input_required.input, Some(json!({"question":"approve?"})));

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
    assert_eq!(resumed.input, Some(json!({"answer":"approved"})));

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
    assert_eq!(delivered.status, TeamActorMessageStatus::Delivered);
    assert!(delivered.delivered_at.is_some());

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
            to_actor_id: "reviewer".to_string(),
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
            to_actor_id: "reviewer".to_string(),
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
async fn actor_mailbox_service_validates_required_fields() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db);
    let service = manager.actor_mailbox_service();

    let err = service
        .actor_send(ActorSendRequest {
            run_id: " ".to_string(),
            from_actor_id: "planner".to_string(),
            from_peer_id: None,
            to_actor_id: "reviewer".to_string(),
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
    assert_eq!(captured[0].body["from_actor_id"], "planner");
    assert_eq!(captured[0].body["from_actor_kind"], "agent");
    assert_eq!(captured[0].body["to_actor_id"], "remote-reviewer");
    assert_eq!(captured[0].body["to_actor_kind"], "agent");
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

    let all_runs = manager
        .list_runs(&team.id, 100, None, None)
        .await
        .expect("list all runs");
    assert_eq!(all_runs.len(), 2);
    assert_eq!(all_runs[0].id, second_run.id);
    assert_eq!(all_runs[1].id, first_run.id);

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
}
