use super::*;

fn canonical(request: char, scope: char) -> McpOperationIntent {
    let mut value = intent(McpReplaySafety::NonIdempotent);
    value.request_key = digest(request);
    value.scope_digest = digest(scope);
    value.scope_identity = McpScopeIdentity::VerifiedAuthority;
    value
}

#[tokio::test]
async fn mcp_scope_legacy_unknown_survives_authority_upgrade_and_late_facts() {
    let mut fixture = Fixture::new().await;
    let old = fixture.running("worker", 100).await;
    let legacy = intent(McpReplaySafety::NonIdempotent);
    let operation = fixture.store.prepare(&old, &legacy, 101).await.unwrap();
    let original_json: String =
        sqlx::query_scalar("SELECT intent_json FROM mcp_operations WHERE id = ?")
            .bind(&operation.id)
            .fetch_one(&fixture.store.pool)
            .await
            .unwrap();
    assert!(!original_json.contains("scope_identity"));
    let permit = fixture
        .store
        .begin_send(&old, &operation.id, 0, 102)
        .await
        .unwrap();
    fixture.stop(&old, 103).await;
    fixture.reopen(true).await;
    assert_eq!(
        fixture.store.recover_interrupted(100, 104).await.unwrap(),
        1
    );
    let current = fixture.running("other", 105).await;
    let mut moved = canonical('a', 'b');
    moved.binding_digest = digest('c');
    moved.schema_digest = digest('d');
    moved.replay_safety = McpReplaySafety::ReadOnly;
    assert_journal_error(
        fixture.store.prepare(&current, &moved, 106).await,
        McpJournalError::UnsafeReplay,
    );
    // Today's credential does not prove the namespace of an old endpoint-only write.
    moved.scope_digest = digest('e');
    assert_journal_error(
        fixture.store.prepare(&current, &moved, 106).await,
        McpJournalError::UnsafeReplay,
    );
    let stored_json: String =
        sqlx::query_scalar("SELECT intent_json FROM mcp_operations WHERE id = ?")
            .bind(&operation.id)
            .fetch_one(&fixture.store.pool)
            .await
            .unwrap();
    assert_eq!(stored_json, original_json);
    let mut unrelated = moved.clone();
    unrelated.server_id = "other-integration".into();
    fixture
        .store
        .prepare(&current, &unrelated, 106)
        .await
        .unwrap();
    fixture
        .store
        .complete(&permit, &success(), 107)
        .await
        .unwrap();
    moved.request_key = digest('f');
    fixture.store.prepare(&current, &moved, 108).await.unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn mcp_scope_upgrade_rechecks_both_directions_at_send() {
    for canonical_first in [false, true] {
        let fixture = Fixture::new().await;
        let executor = fixture.running("worker", 100).await;
        let legacy = intent(McpReplaySafety::NonIdempotent);
        let current = canonical('a', 'b');
        let (first, second) = if canonical_first {
            (&current, &legacy)
        } else {
            (&legacy, &current)
        };
        let first = fixture.store.prepare(&executor, first, 101).await.unwrap();
        let second = fixture.store.prepare(&executor, second, 101).await.unwrap();
        // An array POST cannot commit two sends through the compatibility boundary.
        assert_journal_error(
            fixture
                .store
                .begin_send_batch(&executor, &[(&first.id, 0), (&second.id, 0)], 102)
                .await,
            McpJournalError::InFlight,
        );
        let attempts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mcp_operation_attempts")
            .fetch_one(&fixture.store.pool)
            .await
            .unwrap();
        assert_eq!(attempts, 0);
        let (left, right) = tokio::join!(
            fixture.store.begin_send(&executor, &first.id, 0, 103),
            fixture.store.begin_send(&executor, &second.id, 0, 103),
        );
        assert_ne!(left.is_ok(), right.is_ok());
        assert_journal_error(
            if left.is_err() { left } else { right },
            McpJournalError::InFlight,
        );
        fixture.close().await;
    }
}

#[tokio::test]
async fn mcp_scope_verified_namespaces_remain_distinct_and_aliases_share_effects() {
    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let original = canonical('1', '2');
    let operation = fixture
        .store
        .prepare(&executor, &original, 101)
        .await
        .unwrap();
    let permit = fixture
        .store
        .begin_send(&executor, &operation.id, 0, 102)
        .await
        .unwrap();
    fixture
        .store
        .complete(&permit, &unknown(), 103)
        .await
        .unwrap();
    let mut alias = canonical('a', '2');
    alias.binding_digest = digest('b');
    assert_journal_error(
        fixture.store.prepare(&executor, &alias, 104).await,
        McpJournalError::UnsafeReplay,
    );
    let distinct = canonical('c', 'd');
    let independent = fixture
        .store
        .prepare(&executor, &distinct, 104)
        .await
        .unwrap();
    fixture
        .store
        .begin_send(&executor, &independent.id, 0, 105)
        .await
        .unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn mcp_scope_upgrade_does_not_reassign_a_stable_identity() {
    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let legacy = intent(McpReplaySafety::StableIdentity {
        identity_digest: digest('9'),
    });
    let operation = fixture
        .store
        .prepare(&executor, &legacy, 101)
        .await
        .unwrap();
    let mut current = canonical('a', 'b');
    current.replay_safety = legacy.replay_safety.clone();
    current.arguments_digest = digest('c');
    assert_journal_error(
        fixture.store.prepare(&executor, &current, 102).await,
        McpJournalError::IdentityConflict,
    );
    assert_eq!(
        fixture
            .store
            .prepare(&executor, &legacy, 102)
            .await
            .unwrap(),
        operation
    );
    fixture.close().await;
}

#[tokio::test]
async fn mcp_scope_legacy_reads_do_not_block_a_verified_write() {
    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let legacy = intent(McpReplaySafety::ReadOnly);
    let operation = fixture
        .store
        .prepare(&executor, &legacy, 101)
        .await
        .unwrap();
    fixture
        .store
        .begin_send(&executor, &operation.id, 0, 102)
        .await
        .unwrap();
    let current = canonical('a', 'b');
    fixture
        .store
        .prepare(&executor, &current, 103)
        .await
        .unwrap();
    fixture.close().await;
}
