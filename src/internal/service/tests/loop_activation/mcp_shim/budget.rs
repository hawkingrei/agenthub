use super::*;
use crate::internal::proto::agenthub::internal::v1::ExchangeMcpProxyRequest;

#[tokio::test]
async fn mcp_delivery_budget_pressure_preserves_a_sent_writes_factual_result() {
    let Harness {
        state,
        service,
        authz,
        run,
        reservation,
        journal,
        upstream,
        http,
        ..
    } = setup().await;
    let token = signed_token(
        &authz,
        &reservation,
        &run.id,
        vec![InternalAction::McpProxy.as_str().into()],
    );
    let session_id = service
        .open_mcp_proxy(authenticated_request(
            OpenMcpProxyRequest {
                server_id: "fixture".into(),
            },
            &token,
        ))
        .await
        .unwrap()
        .into_inner()
        .session_id;
    let exchange = |message: Value| {
        service.exchange_mcp_proxy(authenticated_request(
            ExchangeMcpProxyRequest {
                session_id: session_id.clone(),
                message_json: message.to_string(),
            },
            &token,
        ))
    };
    let mut request = discovery::request("tools", "budget-client");
    request["method"] = json!("tools/list");
    let mut stream = exchange(request.clone()).await.unwrap().into_inner();
    assert!(stream.next().await.unwrap().unwrap().finished);
    assert!(stream.next().await.is_none());
    let hub = state.agents.mcp_proxy().unwrap();
    assert_eq!(hub.budget.delivery.used(), 0);
    let pressure = hub
        .budget
        .delivery
        .acquire(8 * agenthub_mcp::MAX_MESSAGE_BYTES)
        .unwrap();
    upstream.hold_writes.store(true, Ordering::Release);
    request["id"] = json!("write");
    request["method"] = json!("tools/call");
    request["params"]["name"] = json!("write");
    request["params"]["arguments"] = json!({"body":"budget-pressure-write"});
    let mut stream = exchange(request).await.unwrap().into_inner();
    tokio::time::timeout(Duration::from_secs(3), upstream.write_received.notified())
        .await
        .unwrap();
    let gate = state
        .agents
        .loop_operation_gate(&reservation.actor_id)
        .await;
    assert!(gate.try_write().is_err());
    upstream.write_release.notify_one();
    assert!(
        tokio::time::timeout(Duration::from_secs(3), stream.next())
            .await
            .unwrap()
            .is_none()
    );
    let events = journal
        .events(
            &run.team_id,
            &reservation.actor_id,
            reservation.activation_id.as_deref().unwrap(),
            0,
            100,
        )
        .await
        .unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(
        events[2].status,
        agenthub_agent_domain::mcp_operations::McpOperationStatus::Succeeded
    );
    assert!(
        !hub.session(&reservation, &session_id)
            .await
            .unwrap()
            .is_active()
    );
    assert_eq!(
        upstream
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|message| message["method"] == "tools/call")
            .count(),
        1
    );
    drop(pressure);
    assert_eq!(hub.budget.delivery.used(), 0);
    state
        .agents
        .daemon_tasks()
        .shutdown_runtime(Duration::from_secs(5))
        .await
        .unwrap();
    assert!(gate.try_write().is_ok());
    http.abort();
}
