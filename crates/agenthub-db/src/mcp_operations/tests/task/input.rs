use super::*;
use agenthub_agent_domain::mcp_operations::{
    McpTaskCancellationInput, McpTaskInputRequest, McpTaskInputResponse, McpTaskUpdateInput,
};

fn request(id: u64) -> McpTaskInputRequest {
    McpTaskInputRequest {
        input_id_digest: hash(id),
        request_digest: hash(id + 1000),
    }
}

fn update(id: u64, inputs: &[u64]) -> McpTaskUpdateInput {
    McpTaskUpdateInput {
        receipt: receipt(),
        request_key: hash(id),
        request_digest: hash(id + 2000),
        inputs: inputs
            .iter()
            .map(|id| McpTaskInputResponse {
                input_id_digest: hash(*id),
                response_digest: hash(id + 3000),
            })
            .collect(),
    }
}

async fn observe(
    f: &Fixture,
    e: &LoopReservation,
    op: &McpOperationRecord,
    id: u64,
    inputs: &[McpTaskInputRequest],
) -> anyhow::Result<()> {
    let permit = f
        .store
        .begin_task_lookup(e, &authority(op), &lookup(id), 104)
        .await?;
    f.store
        .complete_task_lookup_with_inputs(&permit, &success(), None, Some(inputs), 105)
        .await
}

#[tokio::test]
async fn mcp_task_inputs_migrate_and_partial_updates_consume_exactly_once_before_send() {
    let mut f = Fixture::new().await;
    let e = f.running("worker", 100).await;
    let op = pending(&f, &e).await;
    let scope = authority(&op);
    sqlx::query("DROP TABLE mcp_operation_task_inputs")
        .execute(&f.store.pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE mcp_operation_task_updates")
        .execute(&f.store.pool)
        .await
        .unwrap();
    migrate_mcp_operations(&f.store.pool).await.unwrap();
    migrate_mcp_operations(&f.store.pool).await.unwrap();
    observe(&f, &e, &op, 80, &[request(100), request(101)])
        .await
        .unwrap();
    f.reopen(false).await;
    assert_journal_error(
        f.store
            .begin_task_update(&e, &scope, &update(81, &[100, 999]), 106)
            .await,
        McpJournalError::ContinuationRequired,
    );
    assert!(
        f.store
            .task_inputs("team", "worker", &op.id, 0, 100)
            .await
            .unwrap()
            .iter()
            .all(|input| input.update_id.is_none())
    );
    assert!(
        f.store
            .task_updates("team", "worker", &op.id, 0, 100)
            .await
            .unwrap()
            .is_empty()
    );
    let input = update(82, &[100]);
    let (a, b) = tokio::join!(
        f.store.begin_task_update(&e, &scope, &input, 106),
        f.store.begin_task_update(&e, &scope, &input, 106)
    );
    let permit = match (a, b) {
        (Ok(p), Err(_)) | (Err(_), Ok(p)) => p,
        _ => panic!("one update must commit"),
    };
    let updates = f
        .store
        .task_updates("team", "worker", &op.id, 0, 100)
        .await
        .unwrap();
    assert_eq!(updates.len(), 1);
    assert!(updates[0].completion.is_none());
    assert_eq!(updates[0].inputs, input.inputs);
    f.store
        .complete_task_update(&permit, &success(), 107)
        .await
        .unwrap();
    observe(&f, &e, &op, 83, &[request(100), request(101)])
        .await
        .unwrap();
    assert_journal_error(
        f.store
            .begin_task_update(&e, &scope, &update(84, &[100]), 108)
            .await,
        McpJournalError::ContinuationRequired,
    );
    let other = f
        .store
        .begin_task_update(&e, &scope, &update(85, &[101]), 108)
        .await
        .unwrap();
    f.store
        .complete_task_update(&other, &success(), 109)
        .await
        .unwrap();
    assert_eq!(
        f.store
            .task_updates("team", "worker", &op.id, updates[0].sequence, 100)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        f.store
            .task_inputs("team", "other", &op.id, 0, 100)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        f.store
            .task_updates("team", "other", &op.id, 0, 100)
            .await
            .unwrap()
            .is_empty()
    );
    let inputs = f
        .store
        .task_inputs("team", "worker", &op.id, 0, 1)
        .await
        .unwrap();
    assert_eq!(
        f.store
            .task_inputs("team", "worker", &op.id, inputs[0].sequence, 1)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        f.store
            .operation("team", "worker", &op.id)
            .await
            .unwrap()
            .unwrap()
            .completion,
        Some(deferred())
    );
    assert_eq!(
        f.store
            .attempts("team", "worker", &op.id, 0, 100)
            .await
            .unwrap()
            .len(),
        1
    );
    f.close().await;
}

#[tokio::test]
async fn mcp_task_input_equivocation_survives_failed_delivery_and_reopen() {
    let mut f = Fixture::new().await;
    let e = f.running("worker", 100).await;
    let op = pending(&f, &e).await;
    observe(&f, &e, &op, 80, &[request(100)]).await.unwrap();
    let mut changed = request(100);
    changed.request_digest = hash(999);
    assert_journal_error(
        observe(&f, &e, &op, 81, &[changed]).await,
        McpJournalError::TaskInputConflict,
    );
    f.reopen(false).await;
    let inputs = f
        .store
        .task_inputs("team", "worker", &op.id, 0, 100)
        .await
        .unwrap();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].request_digest, request(100).request_digest);
    assert!(inputs[0].conflicted);
    let queries = f
        .store
        .task_lookups("team", "worker", &op.id, 0, 100)
        .await
        .unwrap();
    assert!(matches!(
        queries[1].completion,
        Some(McpCompletion::OutcomeUnknown {
            reason: McpAmbiguityReason::InvalidResponse
        })
    ));
    assert_journal_error(
        f.store
            .begin_task_update(&e, &authority(&op), &update(82, &[100]), 106)
            .await,
        McpJournalError::ContinuationRequired,
    );
    assert_eq!(
        f.store
            .operation("team", "worker", &op.id)
            .await
            .unwrap()
            .unwrap()
            .completion,
        Some(deferred())
    );
    f.close().await;
}

#[tokio::test]
async fn mcp_task_update_restart_never_releases_consumed_inputs_or_completes_the_tool() {
    let mut f = Fixture::new().await;
    let e = f.running("worker", 100).await;
    let op = pending(&f, &e).await;
    let scope = authority(&op);
    observe(&f, &e, &op, 80, &[request(100), request(101)])
        .await
        .unwrap();
    let permit = f
        .store
        .begin_task_update(&e, &scope, &update(81, &[100]), 106)
        .await
        .unwrap();
    f.reopen(true).await;
    assert_eq!(f.store.recover_interrupted(1, 107).await.unwrap(), 1);
    assert_eq!(f.store.recover_interrupted(1, 107).await.unwrap(), 0);
    assert!(matches!(
        f.store
            .task_updates("team", "worker", &op.id, 0, 100)
            .await
            .unwrap()[0]
            .completion,
        Some(McpCompletion::OutcomeUnknown {
            reason: McpAmbiguityReason::DaemonRestart
        })
    ));
    f.stop(&e, 107).await;
    let next = f.running("worker", 108).await;
    assert_journal_error(
        f.store
            .begin_task_update(&next, &scope, &update(82, &[100]), 109)
            .await,
        McpJournalError::ContinuationRequired,
    );
    let other = f
        .store
        .begin_task_update(&next, &scope, &update(83, &[101]), 109)
        .await
        .unwrap();
    let query = f
        .store
        .begin_task_lookup(&next, &scope, &lookup(84), 110)
        .await
        .unwrap();
    f.store
        .complete_task_lookup(&query, &success(), Some(&success()), 111)
        .await
        .unwrap();
    f.stop(&next, 112).await;
    f.store
        .complete_task_update(&permit, &success(), 113)
        .await
        .unwrap();
    f.store
        .complete_task_update(&other, &success(), 113)
        .await
        .unwrap();
    assert_eq!(
        f.store
            .operation("team", "worker", &op.id)
            .await
            .unwrap()
            .unwrap()
            .completion,
        Some(success())
    );
    f.close().await;
}

#[tokio::test]
async fn mcp_task_update_rejects_stale_authority_legacy_version_and_cancelled_intent() {
    let f = Fixture::new().await;
    let e = f.running("worker", 100).await;
    let op = pending(&f, &e).await;
    let scope = authority(&op);
    observe(&f, &e, &op, 80, &[request(100)]).await.unwrap();
    let mut legacy = update(81, &[100]);
    legacy.receipt.version = McpTaskVersion::November2025;
    assert_journal_error(
        f.store.begin_task_update(&e, &scope, &legacy, 106).await,
        McpJournalError::ContinuationRequired,
    );
    let mut other_scope = authority(&op);
    other_scope.binding_digest = hash(999);
    assert!(
        f.store
            .begin_task_update(&e, &other_scope, &update(81, &[100]), 106)
            .await
            .is_err()
    );
    let mut stale = e.clone();
    stale.generation += 1;
    assert!(
        f.store
            .begin_task_update(&stale, &scope, &update(81, &[100]), 106)
            .await
            .is_err()
    );
    f.store
        .begin_task_cancellation(
            &e,
            &scope,
            &McpTaskCancellationInput {
                receipt: receipt(),
                request_key: hash(82),
                request_digest: hash(83),
            },
            106,
        )
        .await
        .unwrap();
    assert_journal_error(
        f.store
            .begin_task_update(&e, &scope, &update(84, &[100]), 107)
            .await,
        McpJournalError::ContinuationRequired,
    );
    assert!(
        f.store
            .task_updates("team", "worker", &op.id, 0, 100)
            .await
            .unwrap()
            .is_empty()
    );
    f.close().await;
}
