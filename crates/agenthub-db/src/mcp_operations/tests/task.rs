use super::continuation::hash;
use super::*;
use agenthub_agent_domain::mcp_operations::{
    McpTaskAuthority, McpTaskLookupInput, McpTaskLookupMethod, McpTaskReceipt, McpTaskVersion,
};

fn receipt() -> McpTaskReceipt {
    McpTaskReceipt {
        task_digest: hash(70),
        version: McpTaskVersion::July2026,
        session_digest: None,
    }
}

fn deferred() -> McpCompletion {
    McpCompletion::Deferred {
        reason: agenthub_agent_domain::mcp_operations::McpDeferralKind::TaskAccepted,
        response_digest: hash(71),
        input_receipt: None,
        task_receipt: Some(receipt()),
    }
}

fn authority(operation: &McpOperationRecord) -> McpTaskAuthority {
    McpTaskAuthority {
        server_id: operation.intent.server_id.clone(),
        scope_digest: operation.intent.scope_digest.clone(),
        binding_digest: operation.intent.binding_digest.clone(),
        tools: std::collections::BTreeMap::from([(
            operation.intent.tool_name.clone(),
            operation.intent.schema_digest.clone(),
        )]),
    }
}

fn lookup(id: u64) -> McpTaskLookupInput {
    McpTaskLookupInput {
        receipt: receipt(),
        method: McpTaskLookupMethod::Get,
        request_key: hash(id),
        request_digest: hash(73),
    }
}

async fn pending(f: &Fixture, e: &LoopReservation) -> McpOperationRecord {
    let operation = f
        .store
        .prepare(e, &intent(McpReplaySafety::NonIdempotent), 101)
        .await
        .unwrap();
    let permit = f.store.begin_send(e, &operation.id, 0, 102).await.unwrap();
    f.store.complete(&permit, &deferred(), 103).await.unwrap();
    operation
}

#[tokio::test]
async fn mcp_task_lookup_migration_and_queries_do_not_repeat_the_tool_send() {
    let mut fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    // Model an earlier control DB, before the additive task tables, with a live send.
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
    sqlx::query("DROP TABLE mcp_operation_task_lookups")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE mcp_operation_tasks")
        .execute(&fixture.store.pool)
        .await
        .unwrap();
    migrate_mcp_operations(&fixture.store.pool).await.unwrap();
    migrate_mcp_operations(&fixture.store.pool).await.unwrap();
    fixture
        .store
        .complete(&permit, &deferred(), 103)
        .await
        .unwrap();
    fixture.reopen(false).await;
    let scope = authority(&operation);
    let query = fixture
        .store
        .begin_task_lookup(&executor, &scope, &lookup(80), 104)
        .await
        .unwrap();
    let records = fixture
        .store
        .task_lookups("team", "worker", &operation.id, 0, 100)
        .await
        .unwrap();
    assert_eq!(records.len(), 1);
    assert!(
        records[0].completion.is_none(),
        "lookup send must commit before I/O"
    );
    fixture
        .store
        .complete_task_lookup(&query, &success(), None, 105)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .store
            .operation("team", "worker", &operation.id)
            .await
            .unwrap()
            .unwrap()
            .completion,
        Some(deferred())
    );
    let query = fixture
        .store
        .begin_task_lookup(&executor, &scope, &lookup(81), 106)
        .await
        .unwrap();
    fixture
        .store
        .complete_task_lookup(&query, &success(), Some(&success()), 107)
        .await
        .unwrap();
    fixture.reopen(false).await;
    let attempts = fixture
        .store
        .attempts("team", "worker", &operation.id, 0, 100)
        .await
        .unwrap();
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].status, McpOperationStatus::Succeeded);
    assert_eq!(
        fixture
            .store
            .task_lookups("team", "worker", &operation.id, records[0].sequence, 100)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        fixture
            .store
            .task_lookups("team", "other", &operation.id, 0, 100)
            .await
            .unwrap()
            .is_empty()
    );
    assert_journal_error(
        fixture.store.complete(&permit, &deferred(), 108).await,
        McpJournalError::StaleAttempt,
    );
    // The actual result can be fetched again, rather than reconstructed from a digest.
    let again = fixture
        .store
        .begin_task_lookup(&executor, &scope, &lookup(82), 108)
        .await
        .unwrap();
    fixture
        .store
        .complete_task_lookup(&again, &success(), Some(&success()), 109)
        .await
        .unwrap();
    fixture.close().await;
}

#[tokio::test]
async fn mcp_task_lookup_revalidates_actor_binding_schema_handle_session_and_generation() {
    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let other = fixture.running("other", 100).await;
    let operation = pending(&fixture, &executor).await;
    for field in [
        "actor",
        "generation",
        "scope",
        "binding",
        "schema",
        "tool",
        "handle",
        "session",
        "version",
        "method",
    ] {
        let mut scope = authority(&operation);
        let mut request = lookup(80);
        let mut active = executor.clone();
        match field {
            "actor" => active = other.clone(),
            "generation" => active.generation += 1,
            "scope" => scope.scope_digest = hash(99),
            "binding" => scope.binding_digest = hash(99),
            "schema" => {
                scope
                    .tools
                    .insert(operation.intent.tool_name.clone(), hash(99));
            }
            "tool" => scope.tools.clear(),
            "handle" => request.receipt.task_digest = hash(99),
            "session" => request.receipt.session_digest = Some(hash(99)),
            "version" => request.receipt.version = McpTaskVersion::November2025,
            _ => request.method = McpTaskLookupMethod::Result,
        }
        assert!(
            fixture
                .store
                .begin_task_lookup(&active, &scope, &request, 104)
                .await
                .is_err(),
            "{field}"
        );
    }
    assert!(
        fixture
            .store
            .task_lookups("team", "worker", &operation.id, 0, 100)
            .await
            .unwrap()
            .is_empty()
    );
    let scope = authority(&operation);
    let request = lookup(80);
    let (first, second) = tokio::join!(
        fixture
            .store
            .begin_task_lookup(&executor, &scope, &request, 104),
        fixture
            .store
            .begin_task_lookup(&executor, &scope, &request, 104)
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    fixture.close().await;
}

#[tokio::test]
async fn mcp_task_lookup_restart_and_late_results_preserve_the_first_terminal_fact() {
    let mut fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let operation = pending(&fixture, &executor).await;
    let scope = authority(&operation);
    let old = fixture
        .store
        .begin_task_lookup(&executor, &scope, &lookup(80), 104)
        .await
        .unwrap();
    fixture.reopen(true).await;
    assert_eq!(fixture.store.recover_interrupted(1, 105).await.unwrap(), 1);
    assert_eq!(fixture.store.recover_interrupted(1, 105).await.unwrap(), 0);
    assert_eq!(
        fixture
            .store
            .operation("team", "worker", &operation.id)
            .await
            .unwrap()
            .unwrap()
            .completion,
        Some(deferred())
    );
    fixture.stop(&executor, 105).await;
    let active = fixture.running("worker", 106).await;
    let query = fixture
        .store
        .begin_task_lookup(&active, &scope, &lookup(81), 107)
        .await
        .unwrap();
    fixture.stop(&active, 108).await;
    fixture
        .store
        .complete_task_lookup(&query, &success(), Some(&success()), 109)
        .await
        .unwrap();
    let failed = McpCompletion::Failed {
        reason: McpFailureKind::JsonRpc,
        response_digest: hash(90),
    };
    fixture
        .store
        .complete_task_lookup(&old, &success(), Some(&failed), 110)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .store
            .operation("team", "worker", &operation.id)
            .await
            .unwrap()
            .unwrap()
            .completion,
        Some(success())
    );
    let records = fixture
        .store
        .task_lookups("team", "worker", &operation.id, 0, 100)
        .await
        .unwrap();
    assert_eq!(records[0].outcome, Some(failed));
    assert_eq!(records[1].activation_id, active.activation_id.unwrap());
    fixture.close().await;
}

#[tokio::test]
async fn mcp_task_lookup_error_does_not_resolve_the_tool_or_authorize_legacy_missing_receipts() {
    let fixture = Fixture::new().await;
    let executor = fixture.running("worker", 100).await;
    let operation = pending(&fixture, &executor).await;
    let query = fixture
        .store
        .begin_task_lookup(&executor, &authority(&operation), &lookup(80), 104)
        .await
        .unwrap();
    let failure = McpCompletion::Failed {
        reason: McpFailureKind::JsonRpc,
        response_digest: hash(90),
    };
    fixture
        .store
        .complete_task_lookup(&query, &failure, None, 105)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .store
            .operation("team", "worker", &operation.id)
            .await
            .unwrap()
            .unwrap()
            .completion,
        Some(deferred())
    );
    assert_journal_error(
        fixture
            .store
            .begin_send(&executor, &operation.id, 1, 106)
            .await,
        McpJournalError::ContinuationRequired,
    );
    let old: McpCompletion = serde_json::from_value(
        serde_json::json!({"kind":"deferred","reason":"task_accepted","response_digest":hash(71)}),
    )
    .unwrap();
    assert!(matches!(
        old,
        McpCompletion::Deferred {
            task_receipt: None,
            ..
        }
    ));
    fixture.close().await;

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
    fixture.store.complete(&permit, &old, 103).await.unwrap();
    assert_journal_error(
        fixture
            .store
            .begin_task_lookup(&executor, &authority(&operation), &lookup(80), 104)
            .await,
        McpJournalError::ContinuationRequired,
    );
    assert!(
        fixture
            .store
            .task_lookups("team", "worker", &operation.id, 0, 100)
            .await
            .unwrap()
            .is_empty()
    );
    fixture.close().await;
}
