use super::*;
use crate::policy::PreparedBatchCall;

fn prepare_batch(
    upstream: &Upstream,
    executor: &LoopReservation,
    offset: i64,
) -> PreparedBatchCall {
    let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
    let catalog =
        McpToolCatalog::from_tools(&catalog().advertised_tools(), ProtocolVersion::March2025)
            .unwrap();
    let mut second = message(offset + 2);
    second["params"]["arguments"]["body"] = json!("second-private-body");
    binding.prepare_batch(Some(&catalog), &McpCallContext {
        executor, proxy_session_id:"batch-session", http:&HttpContext {version:ProtocolVersion::March2025, session_id:None},
    }, json!([message(offset + 1), second, {"jsonrpc":"2.0","id":offset+3,"method":"ping"},
        {"jsonrpc":"2.0","method":"notifications/progress","params":{"progressToken":"local","progress":1}}]),
    |_, _, mut arguments| { arguments["space_id"] = json!("space-a"); Ok(arguments) }).unwrap()
}

#[tokio::test]
async fn one_batch_post_commits_each_out_of_order_tool_result_before_delivery() {
    let fixture = Fixture::new().await;
    let executor = fixture.running().await;
    let upstream = Upstream::new(fixture.pool.clone()).await;
    let call = prepare_batch(&upstream, &executor, 0);
    let client = JournaledMcpClient::new(
        fixture.journal.clone(),
        crate::budget::ByteBudget::new(crate::MAX_MESSAGE_BYTES),
    );
    let (events, mut receiver) = mpsc::channel(8);
    let result = client.run_batch(call, events).await.unwrap();
    assert!(!result.event_delivery_lost);
    assert_eq!(upstream.count(), 1);
    let records = fixture.operations().await;
    assert_eq!(records.len(), 2);
    assert!(
        records
            .iter()
            .all(|record| record.status == McpOperationStatus::Succeeded)
    );
    let response = receiver.recv().await.unwrap().value.message.unwrap();
    assert_eq!(
        response
            .as_array()
            .unwrap()
            .iter()
            .map(|member| member["id"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        vec![3, 2, 1]
    );
    assert_eq!(response[1]["result"]["body"], "second-private-body");
    assert!(receiver.recv().await.is_none());
    drop(upstream);
    fixture.close().await;
}

#[tokio::test]
async fn partial_batch_response_keeps_known_fact_and_blocks_unknown_member_replay() {
    let fixture = Fixture::new().await;
    let executor = fixture.running().await;
    let upstream = Upstream::new(fixture.pool.clone()).await;
    upstream.state.mode.store(8, Ordering::SeqCst);
    let call = prepare_batch(&upstream, &executor, 0);
    let first = call.tools[0].1.request_key.clone();
    let second = call.tools[1].1.request_key.clone();
    let client = JournaledMcpClient::new(
        fixture.journal.clone(),
        crate::budget::ByteBudget::new(crate::MAX_MESSAGE_BYTES),
    );
    let (events, mut receiver) = mpsc::channel(8);
    assert!(matches!(
        client.run_batch(call, events).await,
        Err(McpCallError::Transport(McpTransportError::Disconnected))
    ));
    assert_eq!(
        receiver.recv().await.unwrap().value.message.unwrap()["id"],
        1
    );
    let records = fixture.operations().await;
    assert_eq!(
        records
            .iter()
            .find(|record| record.intent.request_key == first)
            .unwrap()
            .status,
        McpOperationStatus::Succeeded
    );
    assert_eq!(
        records
            .iter()
            .find(|record| record.intent.request_key == second)
            .unwrap()
            .status,
        McpOperationStatus::OutcomeUnknown
    );
    fixture.stop(&executor).await;
    let replacement = fixture.running().await;
    let (events, _receiver) = mpsc::channel(8);
    assert!(matches!(
        client
            .run_batch(prepare_batch(&upstream, &replacement, 10), events)
            .await,
        Err(McpCallError::UnsafeReplay)
    ));
    assert_eq!(upstream.count(), 1);
    let attempts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mcp_operation_attempts")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(attempts, 2);
    drop(upstream);
    fixture.close().await;
}

#[tokio::test]
async fn batch_delivery_pressure_does_not_discard_either_factual_write_result() {
    let fixture = Fixture::new().await;
    let executor = fixture.running().await;
    let upstream = Upstream::new(fixture.pool.clone()).await;
    let client =
        JournaledMcpClient::new(fixture.journal.clone(), crate::budget::ByteBudget::new(1));
    let (events, mut receiver) = mpsc::channel(8);
    let result = client
        .run_batch(prepare_batch(&upstream, &executor, 0), events)
        .await
        .unwrap();
    assert!(result.event_delivery_lost);
    assert!(receiver.recv().await.is_none());
    let records = fixture.operations().await;
    assert_eq!(records.len(), 2);
    assert!(
        records
            .iter()
            .all(|record| record.status == McpOperationStatus::Succeeded)
    );
    assert_eq!(upstream.count(), 1);
    drop(upstream);
    fixture.close().await;
}
