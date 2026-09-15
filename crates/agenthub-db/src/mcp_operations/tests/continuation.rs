use agenthub_agent_domain::mcp_operations::{
    McpContinuationInput, McpDeferralKind, McpInputReceipt,
};

use super::*;

pub(super) fn hash(value: u64) -> McpDigest {
    format!("{value:064x}").try_into().unwrap()
}

pub(super) fn deferred(id: u64) -> McpCompletion {
    McpCompletion::Deferred {
        reason: McpDeferralKind::InputRequired,
        response_digest: hash(1000 + id),
        input_receipt: Some(McpInputReceipt {
            state_digest: Some(hash(8)),
            input_ids: vec![hash(9)],
            request_id_digest: hash(id),
        }),
    }
}

pub(super) fn continuation(
    base: &McpOperationIntent,
    id: u64,
) -> (McpOperationIntent, McpContinuationInput) {
    let mut next = base.clone();
    next.request_key = hash(2000 + id);
    (
        next,
        McpContinuationInput {
            state_digest: Some(hash(8)),
            input_ids: vec![hash(9)],
            request_id_digest: hash(id),
            request_digest: hash(3000 + id),
        },
    )
}

pub(super) async fn parent(
    f: &Fixture,
    e: &LoopReservation,
    safety: McpReplaySafety,
) -> (McpOperationRecord, McpSendPermit) {
    let mut initial = intent(safety);
    initial.request_digest = Some(hash(7));
    let operation = f.store.prepare(e, &initial, 101).await.unwrap();
    let permit = f.store.begin_send(e, &operation.id, 0, 102).await.unwrap();
    f.store.complete(&permit, &deferred(10), 103).await.unwrap();
    (operation, permit)
}

#[tokio::test]
async fn mcp_continuation_migrates_reopens_and_preserves_linked_attempt_history() {
    let mut fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let (operation, original) = parent(&fixture, &executor, McpReplaySafety::NonIdempotent).await;
    // Model a pre-continuation database with an existing operation and immutable receipt.
    sqlx::query("DROP TABLE mcp_operation_continuations")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    migrate_mcp_operations(&fixture.store.pool).await.unwrap();
    migrate_mcp_operations(&fixture.store.pool).await.unwrap();
    fixture.reopen(false).await;
    let (intent, input) = continuation(&operation.intent, 11);
    let permit = fixture
        .store
        .begin_continuation(&executor, &intent, &input, 104)
        .await
        .unwrap();
    assert_eq!(permit.operation_id(), operation.id);
    assert_eq!(permit.attempt_number(), 2);
    assert_journal_error(
        fixture.store.complete(&original, &success(), 105).await,
        McpJournalError::StaleAttempt,
    );
    fixture
        .store
        .complete(&permit, &success(), 105)
        .await
        .unwrap();
    fixture.reopen(false).await;
    let attempts = fixture
        .store
        .attempts("team", "worker", &operation.id, 0, 100)
        .await
        .unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].completion, Some(deferred(10)));
    assert!(attempts[0].continuation.is_none());
    let link = attempts[1].continuation.as_ref().unwrap();
    assert_eq!(link.parent_attempt_number, 1);
    assert_eq!(link.parent_response_digest, hash(1010));
    assert_eq!(link.request_digest, input.request_digest);
    assert_eq!(attempts[1].status, McpOperationStatus::Succeeded);
    assert_eq!(
        fixture
            .store
            .operation("team", "worker", &operation.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        McpOperationStatus::Succeeded
    );
    fixture.close().await;
}

#[tokio::test]
async fn mcp_continuation_is_single_use_and_unknown_round_cannot_restart_original() {
    for safety in [
        McpReplaySafety::ReadOnly,
        McpReplaySafety::NonIdempotent,
        McpReplaySafety::StableIdentity {
            identity_digest: hash(40),
        },
    ] {
        let mut fixture = Fixture::new().await;
        let executor = fixture.running("worker", 100).await;
        let (operation, _) = parent(&fixture, &executor, safety).await;
        let (intent, input) = continuation(&operation.intent, 11);
        let (first, second) = tokio::join!(
            fixture
                .store
                .begin_continuation(&executor, &intent, &input, 104),
            fixture
                .store
                .begin_continuation(&executor, &intent, &input, 104)
        );
        assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
        let permit = first.ok().or_else(|| second.ok()).unwrap();
        fixture.reopen(true).await;
        assert_eq!(
            fixture.store.recover_interrupted(100, 105).await.unwrap(),
            1
        );
        assert_journal_error(
            fixture
                .store
                .begin_send(&executor, &operation.id, 2, 105)
                .await,
            McpJournalError::ContinuationRequired,
        );
        assert_journal_error(
            fixture
                .store
                .begin_continuation(&executor, &intent, &input, 105)
                .await,
            if operation.intent.replay_safety.permits_retry() {
                McpJournalError::IdentityConflict
            } else {
                McpJournalError::UnsafeReplay
            },
        );
        fixture
            .store
            .complete(&permit, &success(), 106)
            .await
            .unwrap();
        assert_eq!(
            fixture
                .store
                .attempts("team", "worker", &operation.id, 0, 100)
                .await
                .unwrap()
                .len(),
            2
        );
        fixture.close().await;
    }
}

#[tokio::test]
async fn mcp_continuation_requires_same_authority_intent_state_and_fresh_request_id() {
    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let other = fixture.running("other", 100).await;
    let (operation, _) = parent(&fixture, &executor, McpReplaySafety::NonIdempotent).await;
    let (intent, input) = continuation(&operation.intent, 11);
    assert_journal_error(
        fixture
            .store
            .begin_continuation(&other, &intent, &input, 104)
            .await,
        McpJournalError::ContinuationRequired,
    );
    for field in ["scope", "binding", "schema", "arguments", "request"] {
        let mut changed = intent.clone();
        match field {
            "scope" => changed.scope_digest = hash(50),
            "binding" => changed.binding_digest = hash(50),
            "schema" => changed.schema_digest = hash(50),
            "arguments" => changed.arguments_digest = hash(50),
            _ => changed.request_digest = Some(hash(50)),
        }
        assert_journal_error(
            fixture
                .store
                .begin_continuation(&executor, &changed, &input, 104)
                .await,
            McpJournalError::ContinuationRequired,
        );
    }
    let mut changed = input.clone();
    changed.state_digest = None;
    assert_journal_error(
        fixture
            .store
            .begin_continuation(&executor, &intent, &changed, 104)
            .await,
        McpJournalError::ContinuationRequired,
    );
    changed = input.clone();
    changed.request_id_digest = hash(10);
    assert_journal_error(
        fixture
            .store
            .begin_continuation(&executor, &intent, &changed, 104)
            .await,
        McpJournalError::IdentityConflict,
    );
    let mut stale = executor.clone();
    stale.generation += 1;
    assert!(
        fixture
            .store
            .begin_continuation(&stale, &intent, &input, 104)
            .await
            .is_err()
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
async fn mcp_continuation_bounds_rounds_and_rejects_missing_legacy_receipt_metadata() {
    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let (operation, _) = parent(&fixture, &executor, McpReplaySafety::NonIdempotent).await;
    for round in 0..10 {
        let id = 11 + round;
        let (intent, input) = continuation(&operation.intent, id);
        let permit = fixture
            .store
            .begin_continuation(&executor, &intent, &input, 104 + round as i64)
            .await
            .unwrap();
        fixture
            .store
            .complete(&permit, &deferred(id), 104 + round as i64)
            .await
            .unwrap();
    }
    let (intent, input) = continuation(&operation.intent, 21);
    assert_journal_error(
        fixture
            .store
            .begin_continuation(&executor, &intent, &input, 115)
            .await,
        McpJournalError::ContinuationRequired,
    );
    let old: McpCompletion = serde_json::from_value(serde_json::json!({"kind":"deferred", "reason":"input_required", "response_digest":hash(1010)})).unwrap();
    assert!(matches!(
        old,
        McpCompletion::Deferred {
            input_receipt: None,
            ..
        }
    ));
    sqlx::query("UPDATE mcp_operations SET completion_json = ? WHERE id = ?")
        .bind(serde_json::to_string(&old).unwrap())
        .bind(&operation.id)
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    assert_journal_error(
        fixture
            .store
            .begin_continuation(&executor, &intent, &input, 115)
            .await,
        McpJournalError::ContinuationRequired,
    );
    fixture.close().await;
}

#[tokio::test]
async fn mcp_continuation_without_state_never_consumes_another_parallel_read_receipt() {
    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let mut operations = Vec::new();
    for index in 0..2 {
        let mut base = intent(McpReplaySafety::ReadOnly);
        base.request_key = hash(100 + index);
        base.request_digest = Some(hash(7));
        let operation = fixture.store.prepare(&executor, &base, 101).await.unwrap();
        let permit = fixture
            .store
            .begin_send(&executor, &operation.id, 0, 102)
            .await
            .unwrap();
        let receipt = McpCompletion::Deferred {
            reason: McpDeferralKind::InputRequired,
            response_digest: hash(200 + index),
            input_receipt: Some(McpInputReceipt {
                state_digest: None,
                input_ids: vec![hash(300 + index)],
                request_id_digest: hash(400 + index),
            }),
        };
        fixture
            .store
            .complete(&permit, &receipt, 103)
            .await
            .unwrap();
        operations.push(operation);
    }
    let (mut next, mut input) = continuation(&operations[0].intent, 500);
    input.state_digest = None;
    input.input_ids.clear();
    assert_journal_error(
        fixture
            .store
            .begin_continuation(&executor, &next, &input, 104)
            .await,
        McpJournalError::ContinuationRequired,
    );
    input.input_ids = vec![hash(300)];
    let first = fixture
        .store
        .begin_continuation(&executor, &next, &input, 104)
        .await
        .unwrap();
    assert_eq!(first.operation_id(), operations[0].id);
    next.request_key = hash(501);
    input.request_id_digest = hash(501);
    assert_journal_error(
        fixture
            .store
            .begin_continuation(&executor, &next, &input, 104)
            .await,
        McpJournalError::ContinuationRequired,
    );
    input.input_ids = vec![hash(301)];
    let second = fixture
        .store
        .begin_continuation(&executor, &next, &input, 104)
        .await
        .unwrap();
    assert_eq!(second.operation_id(), operations[1].id);
    fixture.close().await;
}
