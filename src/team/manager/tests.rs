use super::TeamManager;
use crate::team::{
    TeamActorMessageStatus, TeamActorMessageTransport, TeamDefinitionConfig, TeamRunStatus,
    TeamStepStatus,
};
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

async fn setup_test_db() -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true);
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
        CREATE TABLE team_actor_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT NOT NULL,
            from_actor_id TEXT NOT NULL,
            to_actor_id TEXT NOT NULL,
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
        CREATE UNIQUE INDEX idx_team_actor_messages_idempotency
        ON team_actor_messages(run_id, from_actor_id, idempotency_key)
        WHERE idempotency_key IS NOT NULL
        "#,
    )
    .execute(&pool)
    .await
    .expect("create team_actor_messages idempotency index");

    pool
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

    let row = sqlx::query(
        "SELECT event_type, run_id FROM team_run_events WHERE run_id = ?1 ORDER BY id ASC LIMIT 1",
    )
    .bind(&run.id)
    .fetch_one(&db)
    .await
    .expect("read run event");
    let event_type: String = row.get("event_type");
    let run_id: String = row.get("run_id");
    assert_eq!(event_type, "run_submitted");
    assert_eq!(run_id, run.id);
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
            "run_completed"
        ]
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
        .send_actor_message(
            &run.id,
            "planner",
            "reviewer",
            "coordination",
            TeamActorMessageTransport::Local,
            None,
            json!({"text":"please review"}),
            None,
        )
        .await
        .expect("send message");
    assert_eq!(sent.status, TeamActorMessageStatus::Pending);
    assert_eq!(sent.transport, TeamActorMessageTransport::Local);
    assert_eq!(sent.payload, json!({"text":"please review"}));

    let pending_inbox = manager
        .list_actor_inbox(&run.id, "reviewer", 100, None, false)
        .await
        .expect("list pending inbox");
    assert_eq!(pending_inbox.len(), 1);
    assert_eq!(pending_inbox[0].message_id, sent.message_id);

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
    assert!(pending_after_ack.is_empty());

    let inbox_with_delivered = manager
        .list_actor_inbox(&run.id, "reviewer", 100, None, true)
        .await
        .expect("list inbox with delivered");
    assert_eq!(inbox_with_delivered.len(), 1);
    assert_eq!(
        inbox_with_delivered[0].status,
        TeamActorMessageStatus::Delivered
    );

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
            "actor_message_delivered"
        ]
    );
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
        .send_actor_message(
            &run.id,
            "planner",
            "reviewer",
            "coordination",
            TeamActorMessageTransport::Local,
            None,
            json!({"text":"please review"}),
            Some("msg-1"),
        )
        .await
        .expect("first send");
    let second = manager
        .send_actor_message(
            &run.id,
            "planner",
            "reviewer",
            "coordination",
            TeamActorMessageTransport::Local,
            None,
            json!({"text":"please review"}),
            Some("msg-1"),
        )
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
        .send_actor_message(
            &run.id,
            "planner",
            "reviewer",
            "coordination",
            TeamActorMessageTransport::Local,
            None,
            json!({"text":"please review"}),
            Some("msg-1"),
        )
        .await
        .expect("first send");
    let err = manager
        .send_actor_message(
            &run.id,
            "planner",
            "reviewer",
            "coordination",
            TeamActorMessageTransport::Local,
            None,
            json!({"text":"changed payload"}),
            Some("msg-1"),
        )
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
        .send_actor_message(
            &run.id,
            "planner",
            "remote-reviewer",
            "coordination",
            TeamActorMessageTransport::Remote,
            Some(json!({"endpoint":"mock://ok/remote-reviewer"})),
            json!({"text":"review this"}),
            None,
        )
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

    let relayed = manager
        .list_actor_inbox(&run.id, "remote-reviewer", 100, None, true)
        .await
        .expect("list inbox with delivered");
    assert_eq!(relayed.len(), 1);
    assert_eq!(relayed[0].status, TeamActorMessageStatus::Delivered);
    assert!(relayed[0].delivered_at.is_some());

    let events = manager
        .list_run_events(&run.id, 100, None)
        .await
        .expect("list run events");
    let delivered_count = events
        .iter()
        .filter(|event| event.event_type == "actor_message_delivered")
        .count();
    assert_eq!(delivered_count, 1);
}

#[tokio::test]
async fn remote_actor_messages_relay_supports_retry_and_dead_letter() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

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
        .send_actor_message(
            &run.id,
            "planner",
            "remote-retry",
            "coordination",
            TeamActorMessageTransport::Remote,
            Some(json!({"endpoint":"mock://retry/remote-retry"})),
            json!({"text":"retry this"}),
            None,
        )
        .await
        .expect("send retry remote message");
    let dead_message = manager
        .send_actor_message(
            &run.id,
            "planner",
            "remote-dead",
            "coordination",
            TeamActorMessageTransport::Remote,
            Some(json!({"endpoint":"mock://dead/remote-dead"})),
            json!({"text":"dead-letter this"}),
            None,
        )
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
