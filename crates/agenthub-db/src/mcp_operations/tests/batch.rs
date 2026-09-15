use super::*;

#[tokio::test]
async fn batch_send_admission_rolls_back_every_member_on_failure() {
    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let first = fixture
        .store
        .prepare(&executor, &intent(McpReplaySafety::NonIdempotent), 101)
        .await
        .unwrap();
    let mut second_intent = intent(McpReplaySafety::NonIdempotent);
    second_intent.request_key = digest('6');
    second_intent.arguments_digest = digest('7');
    let second = fixture
        .store
        .prepare(&executor, &second_intent, 101)
        .await
        .unwrap();
    let error = fixture
        .store
        .begin_send_batch(&executor, &[(&first.id, 0), (&second.id, 1)], 102)
        .await
        .err()
        .unwrap();
    assert!(matches!(
        error.downcast_ref::<McpJournalError>(),
        Some(McpJournalError::StaleAttempt)
    ));
    for id in [&first.id, &second.id] {
        let operation = fixture
            .store
            .operation("team", "worker", id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(operation.status, McpOperationStatus::Prepared);
        assert_eq!(operation.attempt_count, 0);
        assert!(
            fixture
                .store
                .attempts("team", "worker", id, 0, 10)
                .await
                .unwrap()
                .is_empty()
        );
    }
    let permits = fixture
        .store
        .begin_send_batch(&executor, &[(&first.id, 0), (&second.id, 0)], 102)
        .await
        .unwrap();
    assert_eq!(permits.len(), 2);
    assert_eq!(permits[0].operation_id(), first.id);
    assert_eq!(permits[1].operation_id(), second.id);
    fixture.close().await;
}

#[tokio::test]
async fn batch_rejects_duplicate_effects_and_preserves_executor_fencing() {
    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let first = fixture
        .store
        .prepare(&executor, &intent(McpReplaySafety::NonIdempotent), 101)
        .await
        .unwrap();
    let mut duplicate = intent(McpReplaySafety::NonIdempotent);
    duplicate.request_key = digest('6');
    let second = fixture
        .store
        .prepare(&executor, &duplicate, 101)
        .await
        .unwrap();
    let error = fixture
        .store
        .begin_send_batch(&executor, &[(&first.id, 0), (&second.id, 0)], 102)
        .await
        .err()
        .unwrap();
    assert!(matches!(
        error.downcast_ref::<McpJournalError>(),
        Some(McpJournalError::InFlight)
    ));
    assert!(
        fixture
            .store
            .attempts("team", "worker", &first.id, 0, 10)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        fixture
            .store
            .begin_send_batch(&executor, &[(&first.id, 0), (&first.id, 0)], 102)
            .await
            .is_err()
    );
    fixture.stop(&executor, 103).await;
    assert!(
        fixture
            .store
            .begin_send_batch(&executor, &[(&first.id, 0)], 104)
            .await
            .is_err()
    );
    assert_eq!(
        fixture
            .store
            .operation("team", "worker", &first.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        McpOperationStatus::Prepared
    );
    fixture.close().await;
}
