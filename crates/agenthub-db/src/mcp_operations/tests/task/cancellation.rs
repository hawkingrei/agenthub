use super::*;
use agenthub_agent_domain::mcp_operations::McpTaskCancellationInput;

fn cancellation(id: u64) -> McpTaskCancellationInput {
    McpTaskCancellationInput {
        receipt: receipt(),
        request_key: hash(id),
        request_digest: hash(91),
    }
}

pub(super) fn cancelled() -> McpCompletion {
    McpCompletion::Failed {
        reason: McpFailureKind::TaskCancelled,
        response_digest: hash(92),
    }
}

pub(super) async fn completion(f: &Fixture, operation: &McpOperationRecord) -> McpCompletion {
    f.store
        .operation("team", "worker", &operation.id)
        .await
        .unwrap()
        .unwrap()
        .completion
        .unwrap()
}

#[tokio::test]
async fn mcp_task_cancellation_commits_once_and_ack_does_not_complete_the_tool() {
    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let operation = pending(&fixture, &executor).await;
    let scope = authority(&operation);
    sqlx::query("DROP TABLE mcp_operation_task_cancellations")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    migrate_mcp_operations(&fixture.store.pool).await.unwrap();
    migrate_mcp_operations(&fixture.store.pool).await.unwrap();
    let mut foreign = cancellation(80);
    foreign.receipt.task_digest = hash(99);
    assert_journal_error(
        fixture
            .store
            .begin_task_cancellation(&executor, &scope, &foreign, 104)
            .await,
        McpJournalError::ContinuationRequired,
    );
    let request = cancellation(80);
    let (a, b) = tokio::join!(
        fixture
            .store
            .begin_task_cancellation(&executor, &scope, &request, 104),
        fixture
            .store
            .begin_task_cancellation(&executor, &scope, &request, 104)
    );
    let permit = match (a, b) {
        (Ok(permit), Err(error)) | (Err(error), Ok(permit)) => {
            assert_eq!(
                error.downcast_ref::<McpJournalError>().unwrap().to_string(),
                McpJournalError::UnsafeReplay.to_string()
            );
            permit
        }
        _ => panic!("exactly one cancellation may send"),
    };
    assert!(
        fixture
            .store
            .task_cancellation("team", "worker", &operation.id, 1)
            .await
            .unwrap()
            .unwrap()
            .completion
            .is_none()
    );
    assert!(
        fixture
            .store
            .task_cancellation("team", "other", &operation.id, 1)
            .await
            .unwrap()
            .is_none()
    );
    fixture
        .store
        .complete_task_cancellation(&permit, &success(), None, 105)
        .await
        .unwrap();
    assert_eq!(completion(&fixture, &operation).await, deferred());
    assert_journal_error(
        fixture
            .store
            .begin_task_cancellation(&executor, &scope, &cancellation(81), 106)
            .await,
        McpJournalError::UnsafeReplay,
    );
    let query = fixture
        .store
        .begin_task_lookup(&executor, &scope, &lookup(82), 106)
        .await
        .unwrap();
    fixture
        .store
        .complete_task_lookup(&query, &success(), Some(&success()), 107)
        .await
        .unwrap();
    assert_journal_error(
        fixture
            .store
            .begin_task_cancellation(&executor, &scope, &cancellation(83), 108)
            .await,
        McpJournalError::AlreadyCompleted,
    );
    assert_eq!(
        fixture
            .store
            .attempts("team", "worker", &operation.id, 0, 100)
            .await
            .unwrap()
            .len(),
        1
    );
    fixture.close().await;
}

#[tokio::test]
async fn mcp_task_cancellation_restart_keeps_intent_and_late_status_cannot_replace_a_result() {
    let mut fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let operation = pending(&fixture, &executor).await;
    let scope = authority(&operation);
    let permit = fixture
        .store
        .begin_task_cancellation(&executor, &scope, &cancellation(80), 104)
        .await
        .unwrap();
    fixture.reopen(true).await;
    assert_eq!(fixture.store.recover_interrupted(1, 105).await.unwrap(), 1);
    assert_eq!(fixture.store.recover_interrupted(1, 105).await.unwrap(), 0);
    assert!(matches!(
        fixture
            .store
            .task_cancellation("team", "worker", &operation.id, 1)
            .await
            .unwrap()
            .unwrap()
            .completion,
        Some(McpCompletion::OutcomeUnknown {
            reason: McpAmbiguityReason::DaemonRestart
        })
    ));
    assert_eq!(completion(&fixture, &operation).await, deferred());
    fixture.stop(&executor, 105).await;
    let active = fixture.running("worker", 106).await;
    assert_journal_error(
        fixture
            .store
            .begin_task_cancellation(&active, &scope, &cancellation(81), 107)
            .await,
        McpJournalError::UnsafeReplay,
    );
    let query = fixture
        .store
        .begin_task_lookup(&active, &scope, &lookup(82), 107)
        .await
        .unwrap();
    fixture
        .store
        .complete_task_lookup(&query, &success(), Some(&success()), 108)
        .await
        .unwrap();
    fixture.stop(&active, 109).await;
    fixture
        .store
        .complete_task_cancellation(&permit, &success(), Some(&cancelled()), 110)
        .await
        .unwrap();
    assert_eq!(completion(&fixture, &operation).await, success());
    assert_eq!(
        fixture
            .store
            .task_cancellation("team", "worker", &operation.id, 1)
            .await
            .unwrap()
            .unwrap()
            .outcome,
        Some(cancelled())
    );
    fixture.close().await;
}

#[tokio::test]
async fn mcp_task_cancellation_status_settles_after_executor_exit_but_rpc_failure_cannot() {
    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let operation = pending(&fixture, &executor).await;
    let permit = fixture
        .store
        .begin_task_cancellation(&executor, &authority(&operation), &cancellation(80), 104)
        .await
        .unwrap();
    let failure = McpCompletion::Failed {
        reason: McpFailureKind::JsonRpc,
        response_digest: hash(93),
    };
    assert_journal_error(
        fixture
            .store
            .complete_task_cancellation(&permit, &failure, Some(&cancelled()), 105)
            .await,
        McpJournalError::ContinuationRequired,
    );
    fixture.stop(&executor, 105).await;
    fixture
        .store
        .complete_task_cancellation(&permit, &success(), Some(&cancelled()), 106)
        .await
        .unwrap();
    fixture
        .store
        .complete_task_cancellation(&permit, &success(), Some(&cancelled()), 107)
        .await
        .unwrap();
    assert_eq!(completion(&fixture, &operation).await, cancelled());
    assert_journal_error(
        fixture
            .store
            .complete_task_cancellation(&permit, &failure, None, 108)
            .await,
        McpJournalError::StaleAttempt,
    );
    fixture.close().await;
}
