use std::path::PathBuf;

use agenthub_agent_domain::{
    loop_runtime::{
        LoopAdmission, LoopCleanupDisposition, LoopLimits, LoopPolicyState, LoopReservation,
        LoopSessionPolicy, LoopSourceReferences, LoopTriggerInput, LoopTriggerKind,
    },
    mcp_operations::{
        McpAmbiguityReason, McpCompletion, McpDigest, McpFailureKind, McpOperationIntent,
        McpOperationStatus, McpReplaySafety,
    },
};
use uuid::Uuid;

use crate::loop_runtime::{LoopPolicyUpdate, LoopStore};

use super::*;

mod batch;
mod continuation;
mod continuation_retry;

struct Fixture {
    path: PathBuf,
    store: McpOperationStore,
    loops: LoopStore,
}

impl Fixture {
    async fn new() -> Self {
        let directory = std::env::temp_dir().join(format!("agenthub-mcp-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("control.sqlite");
        let pool = crate::init_db_at_path(&path).await.unwrap();
        for actor in ["worker", "other"] {
            sqlx::query("INSERT INTO agents(id, name, workdir, command, args, worktree_mode, status, created_at, updated_at) \
                VALUES (?, ?, '/tmp', 'fixture', '[]', 'use_existing', 'created', 1, 1)")
                .bind(actor).bind(actor).execute(&pool).await.unwrap();
        }
        sqlx::query(
            "INSERT INTO team_definitions(id, name, spec_json, created_at, updated_at) \
            VALUES ('team', 'Team', ?, 1, 1)",
        )
        .bind(
            serde_json::json!({"members": [{"member_id": "worker"}, {"member_id": "other"}]})
                .to_string(),
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO team_runs(id, team_id, context_id, status, input_json, created_at) \
            VALUES ('mailbox', 'team', 'loop', 'submitted', '{}', 100)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO loop_mailbox_partitions(run_id, team_id, created_at) VALUES ('mailbox', 'team', 100)")
            .execute(&pool).await.unwrap();
        let daemon = crate::claim_daemon_generation(&pool, "main", "daemon-1", 1, 100)
            .await
            .unwrap();
        let loops = LoopStore::new(pool.clone());
        for actor in ["worker", "other"] {
            loops
                .configure(
                    LoopPolicyUpdate {
                        actor_id: actor,
                        team_id: "team",
                        expected_revision: 0,
                        state: LoopPolicyState::Enabled,
                        session_policy: LoopSessionPolicy::Fresh,
                        limits: &LoopLimits::default(),
                    },
                    100,
                )
                .await
                .unwrap();
        }
        Self {
            path,
            store: McpOperationStore::new(pool, daemon),
            loops,
        }
    }

    async fn running(&self, actor: &str, now: i64) -> LoopReservation {
        let receipt = self
            .loops
            .accept_trigger(
                &LoopTriggerInput {
                    actor_id: actor.into(),
                    team_id: "team".into(),
                    kind: LoopTriggerKind::Operator,
                    source_key: Uuid::new_v4().to_string(),
                    due_at: None,
                    references: LoopSourceReferences::default(),
                },
                now,
            )
            .await
            .unwrap();
        let LoopAdmission::Admitted(reservation) = self
            .loops
            .admit(
                "team",
                &receipt.activation_id,
                &self.store.daemon.owner_id,
                now,
            )
            .await
            .unwrap()
        else {
            panic!("not admitted")
        };
        self.loops
            .bind_mailbox(&reservation, "mailbox", now)
            .await
            .unwrap();
        let session = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at) VALUES (?, ?, 'running', ?)")
            .bind(&session).bind(actor).bind(now).execute(&self.store.pool).await.unwrap();
        let reservation = self
            .loops
            .bind_session(&reservation, &session, now)
            .await
            .unwrap();
        self.loops.mark_running(&reservation, now).await.unwrap();
        reservation
    }

    async fn stop(&self, executor: &LoopReservation, now: i64) {
        self.loops
            .cancel("team", executor.activation_id.as_deref().unwrap(), now)
            .await
            .unwrap();
        self.loops
            .cleanup_verified(executor, LoopCleanupDisposition::Exited, now)
            .await
            .unwrap();
        sqlx::query("UPDATE agent_sessions SET status = 'exited', ended_at = ? WHERE id = ?")
            .bind(now)
            .bind(&executor.session_id)
            .execute(&self.store.pool)
            .await
            .unwrap();
    }

    async fn reopen(&mut self, replace_daemon: bool) {
        let mut daemon = self.store.daemon.clone();
        self.store.pool.close().await;
        let pool = crate::init_db_at_path(&self.path).await.unwrap();
        if replace_daemon {
            daemon =
                crate::claim_daemon_generation(&pool, "main", &Uuid::new_v4().to_string(), 2, 104)
                    .await
                    .unwrap();
        }
        self.loops = LoopStore::new(pool.clone());
        self.store = McpOperationStore::new(pool, daemon);
    }

    async fn close(self) {
        self.store.pool.close().await;
        std::fs::remove_dir_all(self.path.parent().unwrap()).unwrap();
    }
}

fn digest(character: char) -> McpDigest {
    character.to_string().repeat(64).try_into().unwrap()
}

fn intent(safety: McpReplaySafety) -> McpOperationIntent {
    McpOperationIntent {
        request_digest: None,
        request_key: digest('1'),
        server_id: "profile".into(),
        scope_digest: digest('2'),
        binding_digest: digest('3'),
        tool_name: "memory_add".into(),
        schema_digest: digest('4'),
        arguments_digest: digest('5'),
        replay_safety: safety,
    }
}

fn success() -> McpCompletion {
    McpCompletion::Succeeded {
        response_digest: digest('6'),
    }
}

fn unknown() -> McpCompletion {
    McpCompletion::OutcomeUnknown {
        reason: McpAmbiguityReason::TransportLost,
    }
}

fn assert_journal_error<T>(result: anyhow::Result<T>, expected: McpJournalError) {
    let error = result.err().expect("journal rejects operation");
    let error = error
        .downcast_ref::<McpJournalError>()
        .expect("typed journal error");
    assert_eq!(error.to_string(), expected.to_string());
}

#[tokio::test]
async fn mcp_prepared_survives_reopen_and_can_send_in_a_new_activation() {
    let mut fixture = Fixture::new().await;
    let old = fixture.running("worker", 100).await;
    let intent = intent(McpReplaySafety::NonIdempotent);
    let prepared = fixture.store.prepare(&old, &intent, 101).await.unwrap();
    fixture.stop(&old, 102).await;
    fixture.reopen(true).await;
    assert_eq!(
        fixture.store.recover_interrupted(100, 104).await.unwrap(),
        0
    );
    let current = fixture.running("worker", 105).await;
    let replay = fixture.store.prepare(&current, &intent, 106).await.unwrap();
    assert_eq!(replay, prepared);
    let permit = fixture
        .store
        .begin_send(&current, &prepared.id, 0, 106)
        .await
        .unwrap();
    fixture
        .store
        .complete(&permit, &success(), 107)
        .await
        .unwrap();
    fixture.reopen(false).await;
    let record = fixture
        .store
        .operation("team", "worker", &prepared.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.status, McpOperationStatus::Succeeded);
    assert_eq!(record.origin_activation_id, old.activation_id.unwrap());
    let attempts = fixture
        .store
        .attempts("team", "worker", &prepared.id, 0, 100)
        .await
        .unwrap();
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].activation_id, current.activation_id.unwrap());
    fixture.close().await;
}

#[tokio::test]
async fn mcp_sent_restart_blocks_unknown_writes_with_old_or_new_request_ids() {
    let mut fixture = Fixture::new().await;
    let old = fixture.running("worker", 100).await;
    let mut intent = intent(McpReplaySafety::NonIdempotent);
    let operation = fixture.store.prepare(&old, &intent, 101).await.unwrap();
    let _lost = fixture
        .store
        .begin_send(&old, &operation.id, 0, 102)
        .await
        .unwrap();
    // Reopening a database is not proof that another daemon's request has stopped.
    fixture.reopen(false).await;
    assert_eq!(
        fixture.store.recover_interrupted(100, 103).await.unwrap(),
        0
    );
    fixture.stop(&old, 103).await;
    fixture.reopen(true).await;
    assert_eq!(fixture.store.recover_interrupted(1, 104).await.unwrap(), 1);
    assert_eq!(
        fixture.store.recover_interrupted(100, 104).await.unwrap(),
        0
    );
    let current = fixture.running("worker", 105).await;
    let receipt = fixture.store.prepare(&current, &intent, 106).await.unwrap();
    assert_eq!(receipt.status, McpOperationStatus::OutcomeUnknown);
    assert_journal_error(
        fixture
            .store
            .begin_send(&current, &operation.id, 1, 106)
            .await,
        McpJournalError::UnsafeReplay,
    );
    intent.request_key = digest('9');
    intent.server_id = "another-profile-for-the-same-scope".into();
    intent.binding_digest = digest('8');
    intent.schema_digest = digest('7');
    intent.replay_safety = McpReplaySafety::ReadOnly;
    assert_journal_error(
        fixture.store.prepare(&current, &intent, 106).await,
        McpJournalError::UnsafeReplay,
    );
    let other = fixture.running("other", 106).await;
    assert_journal_error(
        fixture.store.prepare(&other, &intent, 107).await,
        McpJournalError::UnsafeReplay,
    );
    assert!(
        fixture
            .store
            .operation("another-team", "worker", &operation.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        fixture
            .store
            .attempts("team", "other", &operation.id, 0, 10)
            .await
            .unwrap()
            .is_empty()
    );
    fixture.close().await;
}

#[tokio::test]
async fn mcp_stable_identity_retries_preserve_intent_and_fence_previous_attempts() {
    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let original = intent(McpReplaySafety::StableIdentity {
        identity_digest: digest('a'),
    });
    let operation = fixture
        .store
        .prepare(&executor, &original, 101)
        .await
        .unwrap();
    let first = fixture
        .store
        .begin_send(&executor, &operation.id, 0, 102)
        .await
        .unwrap();
    fixture
        .store
        .complete(&first, &unknown(), 103)
        .await
        .unwrap();
    let mut changed = original.clone();
    changed.arguments_digest = digest('b');
    assert_journal_error(
        fixture.store.prepare(&executor, &changed, 104).await,
        McpJournalError::IdentityConflict,
    );
    changed.request_key = digest('c');
    assert_journal_error(
        fixture.store.prepare(&executor, &changed, 104).await,
        McpJournalError::IdentityConflict,
    );
    let replay = fixture
        .store
        .prepare(&executor, &original, 104)
        .await
        .unwrap();
    let second = fixture
        .store
        .begin_send(&executor, &operation.id, replay.attempt_count, 104)
        .await
        .unwrap();
    assert_eq!(second.attempt_number(), 2);
    assert_journal_error(
        fixture.store.complete(&first, &success(), 105).await,
        McpJournalError::StaleAttempt,
    );
    fixture
        .store
        .complete(&second, &success(), 106)
        .await
        .unwrap();
    fixture
        .store
        .complete(&second, &success(), 107)
        .await
        .unwrap();
    let attempts = fixture
        .store
        .attempts("team", "worker", &operation.id, 0, 100)
        .await
        .unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].status, McpOperationStatus::OutcomeUnknown);
    assert_eq!(attempts[1].status, McpOperationStatus::Succeeded);
    assert_eq!(
        fixture
            .store
            .attempts("team", "worker", &operation.id, 1, 1)
            .await
            .unwrap(),
        attempts[1..]
    );
    assert_journal_error(
        fixture
            .store
            .begin_send(&executor, &operation.id, 2, 107)
            .await,
        McpJournalError::AlreadyCompleted,
    );
    fixture.close().await;
}

#[tokio::test]
async fn mcp_original_permit_records_a_late_result_after_cleanup_and_restart() {
    let mut fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let operation = fixture
        .store
        .prepare(&executor, &intent(McpReplaySafety::NonIdempotent), 101)
        .await
        .unwrap();
    let permit = fixture
        .store
        .begin_send(&executor, &operation.id, 0, 102)
        .await
        .unwrap();
    fixture.stop(&executor, 103).await;
    fixture.reopen(true).await;
    fixture.store.recover_interrupted(100, 104).await.unwrap();
    fixture
        .store
        .complete(&permit, &success(), 200)
        .await
        .unwrap();
    let activation = executor.activation_id.as_deref().unwrap();
    let events = fixture
        .store
        .events("team", "worker", activation, 0, 100)
        .await
        .unwrap();
    let statuses: Vec<_> = events.iter().map(|event| event.status.as_str()).collect();
    assert_eq!(
        statuses,
        ["prepared", "sent", "outcome_unknown", "succeeded"]
    );
    assert_eq!(
        fixture
            .store
            .events("team", "worker", activation, events[1].id, 1)
            .await
            .unwrap(),
        events[2..3]
    );
    assert!(
        fixture
            .store
            .events("elsewhere", "worker", activation, 0, 100)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        fixture
            .store
            .events("team", "other", activation, 0, 100)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        fixture
            .store
            .begin_send(&executor, &operation.id, 1, 200)
            .await
            .is_err()
    );
    fixture.close().await;
}

#[tokio::test]
async fn mcp_concurrent_requests_receive_only_one_send_permit() {
    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let intent = intent(McpReplaySafety::NonIdempotent);
    let (left, right) = tokio::join!(
        fixture.store.prepare(&executor, &intent, 101),
        fixture.store.prepare(&executor, &intent, 101)
    );
    let operation = left.unwrap();
    assert_eq!(operation, right.unwrap());
    let (left, right) = tokio::join!(
        fixture.store.begin_send(&executor, &operation.id, 0, 102),
        fixture.store.begin_send(&executor, &operation.id, 0, 102)
    );
    assert_ne!(left.is_ok(), right.is_ok());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mcp_operation_attempts")
        .fetch_one(&fixture.store.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    fixture.close().await;
}

#[tokio::test]
async fn mcp_prepared_duplicates_recheck_semantics_at_send_time() {
    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let mut intent = intent(McpReplaySafety::NonIdempotent);
    let first = fixture
        .store
        .prepare(&executor, &intent, 101)
        .await
        .unwrap();
    intent.request_key = digest('7');
    let second = fixture
        .store
        .prepare(&executor, &intent, 101)
        .await
        .unwrap();
    let permit = fixture
        .store
        .begin_send(&executor, &first.id, 0, 102)
        .await
        .unwrap();
    assert_journal_error(
        fixture
            .store
            .begin_send(&executor, &second.id, 0, 102)
            .await,
        McpJournalError::InFlight,
    );
    fixture
        .store
        .complete(&permit, &unknown(), 103)
        .await
        .unwrap();
    assert_journal_error(
        fixture
            .store
            .begin_send(&executor, &second.id, 0, 104)
            .await,
        McpJournalError::UnsafeReplay,
    );
    fixture.close().await;
}

#[tokio::test]
async fn mcp_upstream_errors_are_known_receipts_without_proving_a_write_did_not_happen() {
    for reason in [
        McpFailureKind::JsonRpc,
        McpFailureKind::McpResult,
        McpFailureKind::SuccessEnvelope,
    ] {
        let fixture = Fixture::new().await;
        let executor = fixture.running("worker", 100).await;
        let operation = fixture
            .store
            .prepare(&executor, &intent(McpReplaySafety::NonIdempotent), 101)
            .await
            .unwrap();
        let permit = fixture
            .store
            .begin_send(&executor, &operation.id, 0, 102)
            .await
            .unwrap();
        fixture
            .store
            .complete(
                &permit,
                &McpCompletion::Failed {
                    reason,
                    response_digest: digest('d'),
                },
                103,
            )
            .await
            .unwrap();
        assert_journal_error(
            fixture
                .store
                .begin_send(&executor, &operation.id, 1, 104)
                .await,
            McpJournalError::UnsafeReplay,
        );
        fixture.close().await;
    }
}

#[tokio::test]
async fn mcp_read_retries_and_successful_new_writes_remain_possible() {
    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let read = fixture
        .store
        .prepare(&executor, &intent(McpReplaySafety::ReadOnly), 101)
        .await
        .unwrap();
    let first = fixture
        .store
        .begin_send(&executor, &read.id, 0, 102)
        .await
        .unwrap();
    fixture
        .store
        .complete(&first, &unknown(), 103)
        .await
        .unwrap();
    let second = fixture
        .store
        .begin_send(&executor, &read.id, 1, 104)
        .await
        .unwrap();
    fixture
        .store
        .complete(&second, &success(), 105)
        .await
        .unwrap();
    let mut write = intent(McpReplaySafety::NonIdempotent);
    write.request_key = digest('8');
    let write1 = fixture.store.prepare(&executor, &write, 106).await.unwrap();
    let permit = fixture
        .store
        .begin_send(&executor, &write1.id, 0, 107)
        .await
        .unwrap();
    fixture
        .store
        .complete(&permit, &success(), 108)
        .await
        .unwrap();
    write.request_key = digest('9');
    let write2 = fixture.store.prepare(&executor, &write, 109).await.unwrap();
    assert!(
        fixture
            .store
            .begin_send(&executor, &write2.id, 0, 110)
            .await
            .is_ok()
    );
    fixture.close().await;
}

#[tokio::test]
async fn mcp_send_revalidates_executor_membership_lease_and_daemon() {
    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let operation = fixture
        .store
        .prepare(&executor, &intent(McpReplaySafety::NonIdempotent), 101)
        .await
        .unwrap();
    let mut stale = executor.clone();
    stale.generation += 1;
    assert!(
        fixture
            .store
            .begin_send(&stale, &operation.id, 0, 102)
            .await
            .is_err()
    );
    assert!(
        fixture
            .store
            .begin_send(&executor, &operation.id, 0, 161)
            .await
            .is_err()
    );
    sqlx::query("UPDATE team_definitions SET spec_json = '{\"members\": []}' WHERE id = 'team'")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    assert!(
        fixture
            .store
            .begin_send(&executor, &operation.id, 0, 102)
            .await
            .is_err()
    );
    crate::claim_daemon_generation(&fixture.store.pool, "main", "replacement", 2, 102)
        .await
        .unwrap();
    assert_journal_error(
        fixture
            .store
            .begin_send(&executor, &operation.id, 0, 103)
            .await,
        McpJournalError::StaleDaemon,
    );
    assert_journal_error(
        fixture.store.recover_interrupted(100, 103).await,
        McpJournalError::StaleDaemon,
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mcp_operation_attempts")
        .fetch_one(&fixture.store.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    fixture.close().await;
}

#[tokio::test]
async fn mcp_migration_is_additive_and_idempotent_and_does_not_reconcile_active_sends() {
    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let operation = fixture
        .store
        .prepare(&executor, &intent(McpReplaySafety::NonIdempotent), 101)
        .await
        .unwrap();
    let _permit = fixture
        .store
        .begin_send(&executor, &operation.id, 0, 102)
        .await
        .unwrap();
    migrate_mcp_operations(&fixture.store.pool).await.unwrap();
    migrate_mcp_operations(&fixture.store.pool).await.unwrap();
    assert_eq!(
        fixture
            .store
            .operation("team", "worker", &operation.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        McpOperationStatus::Sent
    );
    let legacy: String = sqlx::query_scalar("SELECT command FROM agents WHERE id = 'worker'")
        .fetch_one(&fixture.store.pool)
        .await
        .unwrap();
    assert_eq!(legacy, "fixture");
    let fields: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('mcp_operations')")
            .fetch_all(&fixture.store.pool)
            .await
            .unwrap();
    assert!(!fields.iter().any(|field| {
        ["arguments", "response", "access_token", "endpoint"].contains(&field.as_str())
    }));
    fixture.close().await;
}

#[test]
fn mcp_journal_boundary_rejects_raw_payloads_and_credentials() {
    let mut original = intent(McpReplaySafety::NonIdempotent);
    original.tool_name = "namespace/tool:version".into();
    original.validate().unwrap();
    assert_eq!(
        serde_json::from_value::<McpOperationIntent>(serde_json::to_value(&original).unwrap())
            .unwrap(),
        original
    );
    for field in ["arguments", "response", "authorization", "endpoint"] {
        let mut value = serde_json::to_value(&original).unwrap();
        value[field] = "sensitive-body".into();
        assert!(serde_json::from_value::<McpOperationIntent>(value).is_err());
    }
    let mut value = serde_json::to_value(&original).unwrap();
    value["arguments_digest"] = "Bearer secret".into();
    assert!(serde_json::from_value::<McpOperationIntent>(value).is_err());
    let mut invalid = original;
    invalid.server_id = "https://secret@example.com/mcp".into();
    assert!(invalid.validate().is_err());
}

#[tokio::test]
async fn mcp_journal_connections_require_full_synchronous_commits() {
    use sqlx::Connection;

    let fixture = Fixture::new().await;
    let mut connection = fixture.store.durable_connection().await.unwrap();
    let mut tx = connection.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let synchronous: i64 = sqlx::query_scalar("PRAGMA synchronous")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(synchronous, 2);
    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(journal_mode, "wal");
    tx.commit().await.unwrap();
    drop(connection);
    fixture.close().await;
}

#[tokio::test]
async fn mcp_abrupt_process_exit_preserves_each_committed_send_boundary() {
    for (phase, expected, recovered) in [
        ("prepared", McpOperationStatus::Prepared, 0),
        ("sent", McpOperationStatus::OutcomeUnknown, 1),
        ("completed", McpOperationStatus::Succeeded, 0),
    ] {
        let mut fixture = Fixture::new().await;
        fixture.running("worker", 100).await;
        fixture.store.pool.close().await;
        let path = fixture.path.clone();
        let output = tokio::task::spawn_blocking(move || {
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "mcp_operations::tests::mcp_crash_child",
                    "--nocapture",
                ])
                .env("AGENTHUB_MCP_CRASH_TEST_DB", path)
                .env("AGENTHUB_MCP_CRASH_TEST_PHASE", phase)
                .output()
                .unwrap()
        })
        .await
        .unwrap();
        assert!(
            output.status.success(),
            "child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        fixture.reopen(true).await;
        assert_eq!(
            fixture.store.recover_interrupted(100, 104).await.unwrap(),
            recovered
        );
        let row = sqlx::query("SELECT * FROM mcp_operations WHERE actor_id = 'worker'")
            .fetch_one(&fixture.store.pool)
            .await
            .unwrap();
        let operation = parse_operation(&row).unwrap();
        assert_eq!(operation.status, expected);
        assert_eq!(operation.intent, intent(McpReplaySafety::NonIdempotent));
        fixture.close().await;
    }
}

#[tokio::test]
async fn mcp_crash_child() {
    let Ok(path) = std::env::var("AGENTHUB_MCP_CRASH_TEST_DB") else {
        return;
    };
    let phase = std::env::var("AGENTHUB_MCP_CRASH_TEST_PHASE").unwrap();
    let pool = crate::init_db_at_path(std::path::Path::new(&path))
        .await
        .unwrap();
    let executor = LoopStore::new(pool.clone())
        .reservation("team", "worker")
        .await
        .unwrap()
        .unwrap();
    let daemon = crate::claim_daemon_generation(
        &pool,
        "main",
        "crash-child",
        i64::from(std::process::id()),
        101,
    )
    .await
    .unwrap();
    let store = McpOperationStore::new(pool, daemon);
    let operation = store
        .prepare(&executor, &intent(McpReplaySafety::NonIdempotent), 101)
        .await
        .unwrap();
    if phase != "prepared" {
        let permit = store
            .begin_send(&executor, &operation.id, 0, 102)
            .await
            .unwrap();
        if phase == "completed" {
            store.complete(&permit, &success(), 103).await.unwrap();
        }
    }
    // Skip Rust destructors, pool shutdown, and SQLite's last-connection checkpoint.
    std::process::exit(0);
}

#[tokio::test]
async fn mcp_deferred_receipt_survives_transport_loss_and_requires_linked_continuation() {
    use agenthub_agent_domain::mcp_operations::McpDeferralKind;

    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let operation = fixture
        .store
        .prepare(&executor, &intent(McpReplaySafety::ReadOnly), 101)
        .await
        .unwrap();
    let permit = fixture
        .store
        .begin_send(&executor, &operation.id, 0, 102)
        .await
        .unwrap();
    let unknown = McpCompletion::OutcomeUnknown {
        reason: McpAmbiguityReason::TransportLost,
    };
    fixture
        .store
        .complete(&permit, &unknown, 103)
        .await
        .unwrap();
    let deferred = McpCompletion::Deferred {
        reason: McpDeferralKind::InputRequired,
        response_digest: "d".repeat(64).try_into().unwrap(),
        input_receipt: None,
    };
    fixture
        .store
        .complete(&permit, &deferred, 104)
        .await
        .unwrap();
    fixture
        .store
        .complete(&permit, &unknown, 105)
        .await
        .unwrap();
    let recorded = fixture
        .store
        .operation("team", "worker", &operation.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recorded.completion, Some(deferred));
    assert_journal_error(
        fixture
            .store
            .begin_send(&executor, &operation.id, 1, 106)
            .await,
        McpJournalError::ContinuationRequired,
    );
    let changed = McpCompletion::Deferred {
        reason: McpDeferralKind::TaskAccepted,
        response_digest: "e".repeat(64).try_into().unwrap(),
        input_receipt: None,
    };
    assert_journal_error(
        fixture.store.complete(&permit, &changed, 106).await,
        McpJournalError::StaleAttempt,
    );
    assert_eq!(
        fixture
            .store
            .events(
                "team",
                "worker",
                executor.activation_id.as_deref().unwrap(),
                0,
                100
            )
            .await
            .unwrap()
            .len(),
        4
    );
    fixture.close().await;
}
