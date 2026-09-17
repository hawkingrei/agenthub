use super::*;
use agenthub_agent_domain::loop_runtime::LoopToolStatus;
use agenthub_agent_domain::mcp_operations::McpDeferralKind;

#[tokio::test]
async fn mcp_tool_history_tracks_sent_deferred_recovered_and_late_facts_without_payloads() {
    let mut fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let id = executor.activation_id.as_deref().unwrap();
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
    let page = fixture
        .loops
        .activation_tool_history("team", "worker", id, None, 100)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(page.tools.len(), 1);
    assert_eq!(page.tools[0].surface.as_str(), "mcp_tool");
    assert_eq!(
        page.tools[0].operation_id.as_deref(),
        Some(operation.id.as_str())
    );
    assert_eq!(page.tools[0].attempt_number, Some(1));
    assert_eq!(page.tools[0].target_ref.as_deref(), Some("profile"));
    assert_eq!(page.tools[0].tool_name, "memory_add");
    assert_eq!(page.tools[0].status, LoopToolStatus::Started);
    assert!(page.tools[0].duration_ms.is_none());
    let deferred = McpCompletion::Deferred {
        reason: McpDeferralKind::InputRequired,
        response_digest: digest('9'),
        input_receipt: None,
        task_receipt: None,
    };
    fixture
        .store
        .complete(&permit, &deferred, 103)
        .await
        .unwrap();
    let mut second_intent = intent(McpReplaySafety::NonIdempotent);
    second_intent.tool_name = "another_tool".into();
    second_intent.request_key = digest('a');
    let second = fixture
        .store
        .prepare(&executor, &second_intent, 103)
        .await
        .unwrap();
    let pending = fixture
        .store
        .begin_send(&executor, &second.id, 0, 103)
        .await
        .unwrap();
    fixture.stop(&executor, 104).await;
    fixture.reopen(true).await;
    fixture.store.recover_interrupted(100, 105).await.unwrap();
    let page = fixture
        .loops
        .activation_tool_history("team", "worker", id, None, 100)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(page.tools[0].status, LoopToolStatus::InputRequired);
    assert!(page.tools[0].duration_ms.is_some());
    assert_eq!(page.tools[1].status, LoopToolStatus::OutcomeUnknown);
    assert!(
        page.tools[1].duration_ms.is_none(),
        "recovery has no surviving process clock"
    );
    let encoded = serde_json::to_string(&page).unwrap();
    for private in [
        "arguments_digest",
        "response_digest",
        "intent_json",
        "completion_json",
        "permit_id",
        "owner_id",
        "input_receipt",
    ] {
        assert!(!encoded.contains(private), "{private}");
    }
    fixture
        .store
        .complete(&pending, &success(), 106)
        .await
        .unwrap();
    let page = fixture
        .loops
        .activation_tool_history("team", "worker", id, None, 100)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(page.tools[1].status, LoopToolStatus::Succeeded);
    assert!(page.tools[1].duration_ms.is_some());
    assert_eq!(
        fixture
            .store
            .attempts("team", "worker", &second.id, 0, 100)
            .await
            .unwrap()
            .len(),
        1
    );
    fixture.close().await;
}

#[tokio::test]
async fn mcp_tool_history_migration_backfills_existing_attempts_once_without_inventing_durations() {
    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let id = executor.activation_id.as_deref().unwrap();
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
        .complete(&permit, &success(), 103)
        .await
        .unwrap();
    let mut input = intent(McpReplaySafety::ReadOnly);
    input.tool_name = "read_tool".into();
    input.request_key = digest('a');
    let operation = fixture.store.prepare(&executor, &input, 104).await.unwrap();
    let _pending = fixture
        .store
        .begin_send(&executor, &operation.id, 0, 104)
        .await
        .unwrap();
    sqlx::query("DROP INDEX idx_mcp_attempt_tool_observation")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE mcp_operation_attempts DROP COLUMN tool_observation_id")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM loop_tool_observations WHERE surface = 'mcp_tool'")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    migrate_mcp_operations(&fixture.store.pool).await.unwrap();
    migrate_mcp_operations(&fixture.store.pool).await.unwrap();
    let page = fixture
        .loops
        .activation_tool_history("team", "worker", id, None, 100)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(page.tools.len(), 2);
    assert!(page.tools.iter().all(|tool| tool.duration_ms.is_none()));
    let completed = page
        .tools
        .iter()
        .find(|tool| tool.tool_name == "memory_add")
        .unwrap();
    assert_eq!(completed.status, LoopToolStatus::Succeeded);
    assert_eq!(completed.started_at, 102);
    assert_eq!(completed.completed_at, Some(103));
    let pending = page
        .tools
        .iter()
        .find(|tool| tool.tool_name == "read_tool")
        .unwrap();
    assert_eq!(pending.status, LoopToolStatus::Started);
    assert_eq!(pending.completed_at, None);
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
    fixture.close().await;
}
