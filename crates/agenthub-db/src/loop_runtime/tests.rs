use std::path::PathBuf;

use agenthub_agent_domain::loop_runtime::{
    LoopActivationState, LoopLimits, LoopPolicyState, LoopSessionPolicy, LoopSourceReferences,
    LoopTriggerInput, LoopTriggerKind,
};
use sqlx::{Row, sqlite::SqlitePoolOptions};
use uuid::Uuid;

use super::{LoopPolicyUpdate, LoopStore, LoopStoreError, migrate_loop_runtime};

#[path = "admission_tests.rs"]
mod admission_tests;
#[path = "history_tests.rs"]
mod history_tests;
#[path = "launch_tests.rs"]
mod launch_tests;
#[path = "lifecycle_tests.rs"]
mod lifecycle_tests;
#[path = "metrics_tests.rs"]
mod metrics_tests;
#[path = "scheduling_tests.rs"]
mod scheduling_tests;
#[path = "scope_tests.rs"]
mod scope_tests;
#[path = "tool_observation_tests.rs"]
mod tool_observation_tests;
#[path = "work_context_tests.rs"]
mod work_context_tests;

struct Fixture {
    path: PathBuf,
    store: LoopStore,
}

impl Fixture {
    async fn new() -> Self {
        let directory = std::env::temp_dir().join(format!("agenthub-loop-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("control.sqlite");
        let pool = crate::init_db_at_path(&path).await.unwrap();
        for actor in ["worker", "other", "outsider"] {
            sqlx::query(
                "INSERT INTO agents(id, name, workdir, command, args, worktree_mode, status, created_at, updated_at) \
                 VALUES (?, ?, '/tmp', 'fixture', '[]', 'use_existing', 'created', 1, 1)",
            ).bind(actor).bind(actor).execute(&pool).await.unwrap();
        }
        for (team, actors) in [
            ("team", vec!["worker", "other"]),
            ("elsewhere", vec!["outsider"]),
        ] {
            let spec = serde_json::json!({"members": actors.into_iter().map(|actor| {
                serde_json::json!({"member_id": actor})
            }).collect::<Vec<_>>()});
            sqlx::query("INSERT INTO team_definitions(id, name, spec_json, created_at, updated_at) VALUES (?, ?, ?, 1, 1)")
                .bind(team).bind(team).bind(spec.to_string()).execute(&pool).await.unwrap();
        }
        Self {
            path,
            store: LoopStore::new(pool),
        }
    }

    async fn enable(&self, actor: &str, limits: &LoopLimits) {
        self.store
            .configure(
                LoopPolicyUpdate {
                    actor_id: actor,
                    team_id: "team",
                    expected_revision: 0,
                    state: LoopPolicyState::Enabled,
                    session_policy: LoopSessionPolicy::Fresh,
                    limits,
                },
                100,
            )
            .await
            .unwrap();
    }

    async fn close(self) {
        self.store.pool.close().await;
        std::fs::remove_dir_all(self.path.parent().unwrap()).unwrap();
    }
}

fn trigger(key: &str) -> LoopTriggerInput {
    LoopTriggerInput {
        actor_id: "worker".into(),
        team_id: "team".into(),
        kind: LoopTriggerKind::Operator,
        source_key: key.into(),
        due_at: None,
        references: LoopSourceReferences::default(),
    }
}

#[tokio::test]
async fn loop_migration_preserves_legacy_settings_without_implicit_opt_in() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql(
        "CREATE TABLE agents(id TEXT PRIMARY KEY, status TEXT, agent_loop_enabled INTEGER); \
         CREATE TABLE team_definitions(id TEXT PRIMARY KEY); \
         CREATE TABLE team_runs(id TEXT PRIMARY KEY); \
         CREATE TABLE agent_sessions(id TEXT PRIMARY KEY); \
         INSERT INTO agents VALUES ('old', 'running', 1);",
    )
    .execute(&pool)
    .await
    .unwrap();
    migrate_loop_runtime(&pool).await.unwrap();
    sqlx::query("DROP INDEX idx_loop_event_activation")
        .execute(&pool)
        .await
        .unwrap();
    migrate_loop_runtime(&pool).await.unwrap();
    let row = sqlx::query("SELECT * FROM agents WHERE id = 'old'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("status"), "running");
    assert_eq!(row.get::<i64, _>("agent_loop_enabled"), 1);
    let policies: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM loop_policies")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(policies, 0);
    let index: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE name = 'idx_loop_event_activation'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(index, 1);
}

#[tokio::test]
async fn loop_trigger_replay_and_trace_survive_database_reopen() {
    let mut fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let input = trigger("operator:1");
    let first = fixture.store.accept_trigger(&input, 100).await.unwrap();
    fixture.store.pool.close().await;
    fixture.store = LoopStore::new(crate::init_db_at_path(&fixture.path).await.unwrap());
    let second = fixture.store.accept_trigger(&input, 200).await.unwrap();
    assert!(!first.duplicate);
    assert!(second.duplicate);
    assert_eq!(first.activation_id, second.activation_id);
    assert_eq!(first.trigger_id, second.trigger_id);
    let activation = fixture
        .store
        .activation("team", &first.activation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(activation.state, LoopActivationState::Pending);
    assert_eq!(activation.due_at, 100);
    assert!(activation.session_id.is_none());
    assert!(activation.mailbox_run_id.is_none());
    let events = fixture
        .store
        .events("team", &first.activation_id, 0, 100)
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].trigger_id.as_deref(),
        Some(first.trigger_id.as_str())
    );
    assert!(
        fixture
            .store
            .events("elsewhere", &first.activation_id, 0, 100)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        fixture
            .store
            .activation("elsewhere", &first.activation_id)
            .await
            .unwrap()
            .is_none()
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_trigger_rejects_conflicting_replay_without_extra_events() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let mut input = trigger("operator:1");
    let first = fixture.store.accept_trigger(&input, 100).await.unwrap();
    input.due_at = Some(1000);
    let error = fixture.store.accept_trigger(&input, 100).await.unwrap_err();
    assert!(matches!(
        error.downcast_ref(),
        Some(LoopStoreError::IdempotencyConflict)
    ));
    assert_eq!(
        fixture
            .store
            .events("team", &first.activation_id, 0, 100)
            .await
            .unwrap()
            .len(),
        1
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_coalescing_preserves_sources_and_future_deadlines() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let first = fixture
        .store
        .accept_trigger(&trigger("first"), 100)
        .await
        .unwrap();
    let second = fixture
        .store
        .accept_trigger(&trigger("second"), 110)
        .await
        .unwrap();
    assert_eq!(first.activation_id, second.activation_id);
    let mut future = trigger("future");
    future.due_at = Some(1000);
    let scheduled = fixture.store.accept_trigger(&future, 110).await.unwrap();
    assert_ne!(first.activation_id, scheduled.activation_id);
    assert_eq!(
        fixture
            .store
            .activation("team", &scheduled.activation_id)
            .await
            .unwrap()
            .unwrap()
            .due_at,
        1000
    );
    assert_eq!(
        fixture
            .store
            .triggers("team", &first.activation_id)
            .await
            .unwrap()
            .len(),
        2
    );
    let events = fixture
        .store
        .events("team", &first.activation_id, 0, 1)
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(
        fixture
            .store
            .events("team", &first.activation_id, events[0].id, 10)
            .await
            .unwrap()
            .len(),
        1
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_policy_revision_and_suspension_preserve_accepted_work() {
    let fixture = Fixture::new().await;
    let disabled = fixture
        .store
        .accept_trigger(&trigger("first"), 100)
        .await
        .unwrap_err();
    assert!(matches!(
        disabled.downcast_ref(),
        Some(LoopStoreError::Disabled)
    ));
    let limits = LoopLimits::default();
    fixture.enable("worker", &limits).await;
    let first = fixture
        .store
        .accept_trigger(&trigger("first"), 100)
        .await
        .unwrap();
    let update = |revision, state| LoopPolicyUpdate {
        actor_id: "worker",
        team_id: "team",
        expected_revision: revision,
        state,
        session_policy: LoopSessionPolicy::Fresh,
        limits: &limits,
    };
    fixture
        .store
        .configure(update(1, LoopPolicyState::Suspended), 101)
        .await
        .unwrap();
    let stale = fixture
        .store
        .configure(update(1, LoopPolicyState::Enabled), 102)
        .await
        .unwrap_err();
    assert!(matches!(
        stale.downcast_ref(),
        Some(LoopStoreError::RevisionConflict)
    ));
    fixture
        .store
        .accept_trigger(&trigger("while-suspended"), 102)
        .await
        .unwrap();
    fixture
        .store
        .configure(update(2, LoopPolicyState::Disabled), 103)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .accept_trigger(&trigger("first"), 104)
            .await
            .unwrap()
            .duplicate
    );
    assert!(
        fixture
            .store
            .accept_trigger(&trigger("new"), 104)
            .await
            .is_err()
    );
    assert_eq!(
        fixture
            .store
            .activation("team", &first.activation_id)
            .await
            .unwrap()
            .unwrap()
            .state,
        LoopActivationState::Pending
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_intake_rejects_wrong_scope_and_stale_membership() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let mut input = trigger("wrong");
    input.team_id = "elsewhere".into();
    let error = fixture.store.accept_trigger(&input, 100).await.unwrap_err();
    assert!(matches!(
        error.downcast_ref(),
        Some(LoopStoreError::ScopeMismatch)
    ));
    let mut input = trigger("bad-reference");
    input.references.scheduling_actor_id = Some("outsider".into());
    assert!(fixture.store.accept_trigger(&input, 100).await.is_err());
    sqlx::query("UPDATE team_definitions SET spec_json = '{}' WHERE id = 'team'")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .accept_trigger(&trigger("removed"), 100)
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM loop_activations")
        .fetch_one(&fixture.store.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    fixture.close().await;
}

#[tokio::test]
async fn loop_capacity_rejects_new_work_without_orphans_or_cross_actor_bypass() {
    let fixture = Fixture::new().await;
    let limits = LoopLimits {
        pending_per_actor: 1,
        pending_per_team: 1,
        sources_per_activation: 1,
        ..LoopLimits::default()
    };
    fixture.enable("worker", &limits).await;
    fixture.enable("other", &LoopLimits::default()).await;
    fixture
        .store
        .accept_trigger(&trigger("first"), 100)
        .await
        .unwrap();
    for actor in ["worker", "other"] {
        let mut input = trigger("excess");
        input.actor_id = actor.into();
        let error = fixture.store.accept_trigger(&input, 101).await.unwrap_err();
        assert!(matches!(
            error.downcast_ref(),
            Some(LoopStoreError::Capacity)
        ));
    }
    assert!(
        fixture
            .store
            .accept_trigger(&trigger("first"), 102)
            .await
            .unwrap()
            .duplicate
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM loop_trigger_sources")
        .fetch_one(&fixture.store.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    fixture.close().await;
}

#[tokio::test]
async fn loop_acceptance_rolls_back_with_canonical_write() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let mut tx = fixture
        .store
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .unwrap();
    sqlx::query("UPDATE agents SET name = 'changed' WHERE id = 'worker'")
        .execute(&mut *tx)
        .await
        .unwrap();
    let receipt = LoopStore::accept_in_transaction(&mut tx, &trigger("atomic"), 100)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    assert!(
        fixture
            .store
            .activation("team", &receipt.activation_id)
            .await
            .unwrap()
            .is_none()
    );
    let name: String = sqlx::query_scalar("SELECT name FROM agents WHERE id = 'worker'")
        .fetch_one(&fixture.store.pool)
        .await
        .unwrap();
    assert_eq!(name, "worker");
    fixture.close().await;
}

#[tokio::test]
async fn loop_concurrent_delivery_has_one_durable_receipt() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let input = trigger("duplicate");
    let (left, right) = tokio::join!(
        fixture.store.accept_trigger(&input, 100),
        fixture.store.accept_trigger(&input, 100),
    );
    let (left, right) = (left.unwrap(), right.unwrap());
    assert_eq!(left.activation_id, right.activation_id);
    assert_eq!(left.trigger_id, right.trigger_id);
    assert_ne!(left.duplicate, right.duplicate);
    fixture.close().await;
}

#[test]
fn loop_intake_schema_excludes_arbitrary_payloads() {
    let mut value = serde_json::to_value(trigger("valid")).unwrap();
    value["prompt"] = serde_json::json!("private message body");
    assert!(serde_json::from_value::<LoopTriggerInput>(value).is_err());
    let mut input = trigger("private message body");
    assert!(input.validate().is_err());
    input.source_key = "valid".into();
    input.references.task_id = Some("private\nbody".into());
    assert!(input.validate().is_err());
}

#[tokio::test]
async fn loop_reference_checks_preserve_task_message_and_scheduler_scope() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    sqlx::raw_sql(
        "INSERT INTO team_tasks(id, team_id, title, status, created_by_actor_id, assigned_member_id, context_json, created_at, updated_at) \
         VALUES ('task', 'team', 'Fixture', 'open', 'worker', 'worker', '{}', 1, 1); \
         INSERT INTO team_runs(id, team_id, context_id, status, input_json, created_at) \
         VALUES ('run', 'team', 'context', 'working', '{}', 1); \
         INSERT INTO team_conversations(id, team_id, task_id, mode, created_at, updated_at) \
         VALUES ('conversation', 'team', 'task', 'group_chat', 1, 1); \
         INSERT INTO team_conversation_messages(id, conversation_id, task_id, from_actor_id, route, payload_json, created_at) \
         VALUES (123, 'conversation', 'task', 'worker', 'group', '{}', 1); \
         INSERT INTO team_actor_messages(id, run_id, from_actor_id, to_actor_id, channel, transport, payload_json, status, created_at) \
         VALUES (456, 'run', 'other', 'worker', 'default', 'local', '{}', 'pending', 1);",
    )
    .execute(&fixture.store.pool)
    .await
    .unwrap();
    let origin = fixture
        .store
        .accept_trigger(&trigger("origin"), 100)
        .await
        .unwrap();
    let mut input = trigger("references");
    input.references = LoopSourceReferences {
        task_id: Some("task".into()),
        mailbox_message_id: Some(456),
        conversation_message_id: Some(123),
        thread_id: Some(123),
        scheduling_actor_id: Some("worker".into()),
        scheduling_activation_id: Some(origin.activation_id),
        scheduling_user_id: None,
        app_id: None,
        app_event: None,
    };
    fixture.store.accept_trigger(&input, 100).await.unwrap();
    let mut invalid = input.clone();
    invalid.source_key = "wrong-task".into();
    invalid.references.task_id = Some("absent".into());
    assert!(fixture.store.accept_trigger(&invalid, 100).await.is_err());
    invalid = input.clone();
    invalid.source_key = "wrong-mailbox".into();
    invalid.references.mailbox_message_id = Some(123);
    assert!(fixture.store.accept_trigger(&invalid, 100).await.is_err());
    invalid = input.clone();
    invalid.source_key = "wrong-conversation".into();
    invalid.references.conversation_message_id = Some(456);
    assert!(fixture.store.accept_trigger(&invalid, 100).await.is_err());
    invalid = input;
    invalid.source_key = "wrong-scheduler".into();
    invalid.references.scheduling_actor_id = Some("other".into());
    assert!(fixture.store.accept_trigger(&invalid, 100).await.is_err());
    fixture.close().await;
}

#[tokio::test]
async fn loop_source_foreign_key_rejects_a_different_activation_actor() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    fixture.enable("other", &LoopLimits::default()).await;
    let first = fixture
        .store
        .accept_trigger(&trigger("first"), 100)
        .await
        .unwrap();
    let error = sqlx::query(
        "INSERT INTO loop_trigger_sources(id, activation_id, actor_id, team_id, source_kind, source_key, input_json, created_at) \
         VALUES ('invalid', ?, 'other', 'team', 'operator', 'key', '{}', 100)",
    ).bind(first.activation_id).execute(&fixture.store.pool).await.unwrap_err();
    assert!(
        error
            .as_database_error()
            .unwrap()
            .is_foreign_key_violation()
    );
    fixture.close().await;
}
