use super::*;

pub(super) async fn awaiting_initialized(session: &McpProxySession) {
    let mut protocol = session.protocol.lock().await;
    protocol.begin(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-03-26","capabilities":{"roots":{}},"clientInfo":{"name":"fixture","version":"1"}}})).unwrap();
    protocol.accept_initialize_response(&json!({"jsonrpc":"2.0","id":1,"result":{
        "protocolVersion":"2025-03-26","capabilities":{},"serverInfo":{"name":"fixture","version":"1"}}}), None).unwrap();
}

#[tokio::test]
async fn task_methods_cannot_bypass_receipt_admission_inside_legacy_batches() {
    let session = session();
    awaiting_initialized(&session).await;
    for method in ["tasks/get", "tasks/result", "tasks/cancel", "tasks/update"] {
        assert!(
            session
                .prepare(
                    &executor(),
                    json!([
                        {"jsonrpc":"2.0","method":"notifications/initialized"},
                        {"jsonrpc":"2.0","id":2,"method":method,"params":{"taskId":"foreign"}}
                    ])
                )
                .await
                .is_err()
        );
        assert!(session.request_ids.lock().await.is_empty());
        assert!(session.protocol.lock().await.awaiting_initialized());
    }
}

#[tokio::test]
async fn prepared_initialized_notification_does_not_enable_listener_before_delivery() {
    let session = session();
    awaiting_initialized(&session).await;
    let pending = session
        .prepare(
            &executor(),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        )
        .await
        .unwrap();
    assert!(!session.can_listen().await);
    drop(pending);
    assert!(session.prepare_listener().await.is_err());
}

#[tokio::test]
async fn failed_batch_does_not_advance_initialization_or_consume_request_ids() {
    let session = session();
    awaiting_initialized(&session).await;
    let initialized = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
    let ping = json!({"jsonrpc":"2.0","id":2,"method":"ping"});
    let tool = json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"missing","arguments":{}}});
    assert!(matches!(
        session
            .prepare(&executor(), json!([initialized, ping, tool]))
            .await,
        Err(McpPolicyError::ToolNotAvailable)
    ));
    assert!(session.request_ids.lock().await.is_empty());
    assert!(
        session
            .protocol
            .lock()
            .await
            .begin(&json!({"jsonrpc":"2.0","id":4,"method":"tools/list"}))
            .is_err()
    );
    assert!(
        session
            .prepare(&executor(), json!([initialized, ping]))
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn callback_batch_checks_all_ids_atomically_and_bypasses_lifecycle_gate() {
    let session = session();
    awaiting_initialized(&session).await;
    session
        .observe(&json!([
            {"jsonrpc":"2.0","id":"a","method":"roots/list"},
            {"jsonrpc":"2.0","id":"b","method":"roots/list"}
        ]))
        .await
        .unwrap();
    let gate = session.lifecycle_gate.lock().await;
    let ordinary_slots = session
        .control_slots
        .clone()
        .acquire_many_owned(8)
        .await
        .unwrap();
    let response = |id| json!({"jsonrpc":"2.0","id":id,"result":{"roots":[]}});
    for message in [
        json!([response("a"), response("unknown")]),
        json!([response("a"), response("a")]),
    ] {
        assert!(matches!(
            session.prepare(&executor(), message).await,
            Err(McpPolicyError::Call)
        ));
        assert_eq!(session.callbacks.lock().await.len(), 2);
    }
    let prepared = tokio::time::timeout(
        Duration::from_secs(1),
        session.prepare(&executor(), json!([response("b"), response("a")])),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(session.callbacks.lock().await.is_empty());
    drop((gate, ordinary_slots, prepared));
}

#[tokio::test]
async fn batch_members_share_capacity_and_rejected_duplicates_do_not_poison_next_batch() {
    let session = session();
    awaiting_initialized(&session).await;
    session
        .protocol
        .lock()
        .await
        .begin(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .unwrap();
    let ping = |id| json!({"jsonrpc":"2.0","id":id,"method":"ping"});
    assert!(
        session
            .prepare(&executor(), json!([ping(2), ping(2)]))
            .await
            .is_err()
    );
    let full = Value::Array((2..10).map(ping).collect());
    let prepared = session.prepare(&executor(), full).await.unwrap();
    assert!(session.prepare(&executor(), ping(10)).await.is_err());
    drop(prepared);
    assert!(session.prepare(&executor(), ping(10)).await.is_ok());
}
