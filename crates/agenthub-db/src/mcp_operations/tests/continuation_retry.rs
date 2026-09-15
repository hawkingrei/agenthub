use super::continuation::{continuation, deferred, hash, parent};
use super::*;

fn unknown() -> McpCompletion {
    McpCompletion::OutcomeUnknown {
        reason: McpAmbiguityReason::TransportLost,
    }
}

#[tokio::test]
async fn mcp_continuation_retry_cannot_consume_another_receipt_with_matching_extra_inputs() {
    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let (operation, _) = parent(&fixture, &executor, McpReplaySafety::ReadOnly).await;
    let (intent, mut input) = continuation(&operation.intent, 11);
    // Opaque state uniquely selects the first receipt, so extra input IDs reach upstream.
    input.input_ids = vec![hash(50)];
    let permit = fixture
        .store
        .begin_continuation(&executor, &intent, &input, 104)
        .await
        .unwrap();
    fixture
        .store
        .complete(&permit, &unknown(), 104)
        .await
        .unwrap();
    let mut other_intent = operation.intent.clone();
    other_intent.request_key = hash(60);
    let other = fixture
        .store
        .prepare(&executor, &other_intent, 104)
        .await
        .unwrap();
    let other_permit = fixture
        .store
        .begin_send(&executor, &other.id, 0, 104)
        .await
        .unwrap();
    let mut other_receipt = deferred(61);
    if let McpCompletion::Deferred {
        input_receipt: Some(receipt),
        ..
    } = &mut other_receipt
    {
        receipt.input_ids = input.input_ids.clone();
    }
    fixture
        .store
        .complete(&other_permit, &other_receipt, 104)
        .await
        .unwrap();
    let (retry_intent, mut retry) = continuation(&operation.intent, 12);
    retry.request_digest = input.request_digest;
    retry.input_ids = input.input_ids;
    assert_journal_error(
        fixture
            .store
            .begin_continuation(&executor, &retry_intent, &retry, 104)
            .await,
        McpJournalError::ContinuationRequired,
    );
    assert_eq!(
        fixture
            .store
            .attempts("team", "worker", &operation.id, 0, 100)
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        fixture
            .store
            .attempts("team", "worker", &other.id, 0, 100)
            .await
            .unwrap()
            .len(),
        1
    );
    fixture.close().await;
}

#[tokio::test]
async fn mcp_continuation_retry_migrates_recovers_and_keeps_the_original_round() {
    for safety in [
        McpReplaySafety::ReadOnly,
        McpReplaySafety::StableIdentity {
            identity_digest: hash(40),
        },
    ] {
        let mut fixture = Fixture::new().await;
        let executor = fixture.running("worker", 100).await;
        let (operation, _) = parent(&fixture, &executor, safety).await;
        let (intent, input) = continuation(&operation.intent, 11);
        let original = fixture
            .store
            .begin_continuation(&executor, &intent, &input, 104)
            .await
            .unwrap();
        // Existing continuation rows survive an additive upgrade from the previous schema.
        sqlx::query("DROP TABLE mcp_operation_continuation_retries")
            .execute(&fixture.store.pool)
            .await
            .unwrap();
        migrate_mcp_operations(&fixture.store.pool).await.unwrap();
        migrate_mcp_operations(&fixture.store.pool).await.unwrap();
        fixture.reopen(true).await;
        assert_eq!(
            fixture.store.recover_interrupted(100, 105).await.unwrap(),
            1
        );
        fixture.stop(&executor, 105).await;
        let resumed = fixture.running("worker", 106).await;
        let (next, mut retry) = continuation(&operation.intent, 12);
        retry.request_digest = input.request_digest.clone();
        let (alternate, mut alternate_input) = continuation(&operation.intent, 13);
        alternate_input.request_digest = input.request_digest.clone();
        let (first, second) = tokio::join!(
            fixture
                .store
                .begin_continuation(&resumed, &next, &retry, 107),
            fixture
                .store
                .begin_continuation(&resumed, &alternate, &alternate_input, 107)
        );
        assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
        let permit = first.ok().or_else(|| second.ok()).unwrap();
        assert_eq!(permit.operation_id(), operation.id);
        assert_eq!(permit.attempt_number(), 3);
        assert_journal_error(
            fixture.store.complete(&original, &success(), 108).await,
            McpJournalError::StaleAttempt,
        );
        fixture
            .store
            .complete(&permit, &unknown(), 108)
            .await
            .unwrap();
        fixture.reopen(false).await;
        let (last, mut last_input) = continuation(&operation.intent, 14);
        last_input.request_digest = input.request_digest.clone();
        let last_permit = fixture
            .store
            .begin_continuation(&resumed, &last, &last_input, 109)
            .await
            .unwrap();
        fixture
            .store
            .complete(&last_permit, &deferred(14), 110)
            .await
            .unwrap();
        let (next_round, next_input) = continuation(&operation.intent, 15);
        let next_permit = fixture
            .store
            .begin_continuation(&resumed, &next_round, &next_input, 111)
            .await
            .unwrap();
        fixture
            .store
            .complete(&next_permit, &success(), 112)
            .await
            .unwrap();
        fixture.reopen(false).await;
        let attempts = fixture
            .store
            .attempts("team", "worker", &operation.id, 0, 100)
            .await
            .unwrap();
        assert_eq!(attempts.len(), 5);
        assert!(matches!(
            attempts[1].completion,
            Some(McpCompletion::OutcomeUnknown {
                reason: McpAmbiguityReason::DaemonRestart
            })
        ));
        for attempt in &attempts[2..4] {
            let link = attempt.continuation.as_ref().unwrap();
            assert_eq!(link.parent_attempt_number, 1);
            assert_eq!(link.parent_response_digest, hash(1010));
            assert_eq!(link.retry_of_attempt_number, Some(2));
            assert_eq!(link.request_digest, input.request_digest);
            assert_eq!(
                attempt.activation_id,
                resumed.activation_id.as_ref().unwrap().as_str()
            );
        }
        assert_eq!(
            attempts[3].continuation.as_ref().unwrap().request_id_digest,
            last_input.request_id_digest
        );
        let link = attempts[4].continuation.as_ref().unwrap();
        assert_eq!(link.parent_attempt_number, 4);
        assert_eq!(link.retry_of_attempt_number, None);
        assert_eq!(attempts[4].status, McpOperationStatus::Succeeded);
        fixture.close().await;
    }
}

#[tokio::test]
async fn mcp_continuation_retry_requires_unchanged_round_policy_and_live_authority() {
    for safety in [
        McpReplaySafety::NonIdempotent,
        McpReplaySafety::StableIdentity {
            identity_digest: hash(40),
        },
    ] {
        let fixture = Fixture::new().await;
        let executor = fixture.running("worker", 100).await;
        let other = fixture.running("other", 100).await;
        let (operation, _) = parent(&fixture, &executor, safety).await;
        let (intent, input) = continuation(&operation.intent, 11);
        let original = fixture
            .store
            .begin_continuation(&executor, &intent, &input, 104)
            .await
            .unwrap();
        fixture
            .store
            .complete(&original, &unknown(), 105)
            .await
            .unwrap();
        let (next, mut retry) = continuation(&operation.intent, 12);
        retry.request_digest = input.request_digest.clone();
        for field in [
            "scope",
            "binding",
            "schema",
            "arguments",
            "request",
            "policy",
            "round",
            "state",
            "actor",
            "generation",
        ] {
            let mut changed = next.clone();
            let mut changed_input = retry.clone();
            let mut authority = executor.clone();
            match field {
                "scope" => changed.scope_digest = hash(99),
                "binding" => changed.binding_digest = hash(99),
                "schema" => changed.schema_digest = hash(99),
                "arguments" => changed.arguments_digest = hash(99),
                "request" => changed.request_digest = Some(hash(99)),
                "policy" => changed.replay_safety = McpReplaySafety::ReadOnly,
                "round" => changed_input.request_digest = hash(99),
                "state" => changed_input.state_digest = Some(hash(99)),
                "actor" => authority = other.clone(),
                _ => authority.generation += 1,
            }
            assert!(
                fixture
                    .store
                    .begin_continuation(&authority, &changed, &changed_input, 106)
                    .await
                    .is_err(),
                "{field}"
            );
        }
        assert_journal_error(
            fixture
                .store
                .begin_send(&executor, &operation.id, 2, 106)
                .await,
            McpJournalError::ContinuationRequired,
        );
        if operation.intent.replay_safety.permits_retry() {
            assert_journal_error(
                fixture
                    .store
                    .begin_continuation(&executor, &intent, &input, 106)
                    .await,
                McpJournalError::IdentityConflict,
            );
            let permit = fixture
                .store
                .begin_continuation(&executor, &next, &retry, 106)
                .await
                .unwrap();
            fixture
                .store
                .complete(
                    &permit,
                    &McpCompletion::Failed {
                        reason: McpFailureKind::JsonRpc,
                        response_digest: hash(60),
                    },
                    107,
                )
                .await
                .unwrap();
            assert_journal_error(
                fixture
                    .store
                    .begin_continuation(&executor, &next, &retry, 108)
                    .await,
                McpJournalError::IdentityConflict,
            );
            let (last, mut last_input) = continuation(&operation.intent, 13);
            last_input.request_digest = input.request_digest.clone();
            let permit = fixture
                .store
                .begin_continuation(&executor, &last, &last_input, 108)
                .await
                .unwrap();
            fixture
                .store
                .complete(&permit, &success(), 109)
                .await
                .unwrap();
        } else {
            assert_journal_error(
                fixture
                    .store
                    .begin_continuation(&executor, &next, &retry, 106)
                    .await,
                McpJournalError::UnsafeReplay,
            );
            assert_eq!(
                fixture
                    .store
                    .attempts("team", "worker", &operation.id, 0, 100)
                    .await
                    .unwrap()
                    .len(),
                2
            );
        }
        fixture.close().await;
    }
}

#[tokio::test]
async fn mcp_continuation_retry_budget_is_per_round_and_cannot_hide_extra_sends() {
    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let (operation, _) = parent(&fixture, &executor, McpReplaySafety::ReadOnly).await;
    for round in 0..10 {
        let id = 100 + round * 10;
        let (intent, input) = continuation(&operation.intent, id);
        let mut permit = fixture
            .store
            .begin_continuation(&executor, &intent, &input, 104)
            .await
            .unwrap();
        for retry_id in 1..=3 {
            fixture
                .store
                .complete(&permit, &unknown(), 104)
                .await
                .unwrap();
            let (next, mut retry) = continuation(&operation.intent, id + retry_id);
            retry.request_digest = input.request_digest.clone();
            permit = fixture
                .store
                .begin_continuation(&executor, &next, &retry, 104)
                .await
                .unwrap();
        }
        fixture
            .store
            .complete(&permit, &unknown(), 104)
            .await
            .unwrap();
        let (extra, mut extra_input) = continuation(&operation.intent, id + 4);
        extra_input.request_digest = input.request_digest.clone();
        assert_journal_error(
            fixture
                .store
                .begin_continuation(&executor, &extra, &extra_input, 104)
                .await,
            McpJournalError::ContinuationRequired,
        );
        // A factual result can still arrive after the retry budget is exhausted.
        fixture
            .store
            .complete(&permit, &deferred(id + 3), 104)
            .await
            .unwrap();
    }
    let (intent, input) = continuation(&operation.intent, 999);
    assert_journal_error(
        fixture
            .store
            .begin_continuation(&executor, &intent, &input, 104)
            .await,
        McpJournalError::ContinuationRequired,
    );
    let attempts = fixture
        .store
        .attempts("team", "worker", &operation.id, 0, 100)
        .await
        .unwrap();
    assert_eq!(attempts.len(), 41);
    assert_eq!(
        attempts
            .iter()
            .filter(|attempt| attempt
                .continuation
                .as_ref()
                .is_some_and(|link| link.retry_of_attempt_number.is_some()))
            .count(),
        30
    );
    fixture.close().await;
}
