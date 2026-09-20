use super::*;
use crate::internal::proto::agenthub::internal::v1::{
    CloseMcpProxyRequest, ExchangeMcpProxyRequest,
};
use agenthub_agent_domain::loop_runtime::LoopLaunchSnapshot;

#[tokio::test]
async fn mcp_bootstrap_allows_list_subscriptions_but_rejects_task_and_resource_access() {
    let Harness {
        state,
        service,
        authz,
        run,
        reservation,
        upstream,
        http,
        ..
    } = setup_with_running(false).await;
    LoopStore::new(state.db.clone())
        .record_launch(
            &reservation,
            &LoopLaunchSnapshot {
                version: 1,
                provider_id: "fixture".into(),
                configuration_digest: "a".repeat(64),
                entry_prompt_version: crate::acp::LOOP_ACTIVATION_CONTRACT_VERSION.into(),
                session_policy: LoopSessionPolicy::Fresh,
                workspace: "/fixture".into(),
                model: None,
                thinking_level: None,
            },
            chrono::Utc::now().timestamp(),
        )
        .await
        .unwrap();
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
    let exchange = |filter: Value| {
        let message = json!({"jsonrpc":"2.0","id":1,"method":"subscriptions/listen","params":{"notifications":filter,
            "_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}});
        service.exchange_mcp_proxy(authenticated_request(
            ExchangeMcpProxyRequest {
                session_id: session_id.clone(),
                message_json: message.to_string(),
            },
            &token,
        ))
    };
    for filter in [
        json!({"taskIds":["private-task"]}),
        json!({"resourceSubscriptions":["private://resource"]}),
    ] {
        assert_eq!(
            exchange(filter).await.err().unwrap().code(),
            tonic::Code::PermissionDenied
        );
    }
    assert!(upstream.calls.lock().unwrap().is_empty());
    let mut stream = exchange(json!({"toolsListChanged":true}))
        .await
        .unwrap()
        .into_inner();
    let frame = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&frame.message_json).unwrap()["method"],
        "notifications/subscriptions/acknowledged"
    );
    let gate = state
        .agents
        .loop_operation_gate(&reservation.actor_id)
        .await;
    let writer = tokio::time::timeout(Duration::from_millis(500), gate.write())
        .await
        .expect("bootstrap subscription retained executor cleanup guard");
    drop(writer);
    tokio::time::timeout(
        Duration::from_secs(2),
        service.close_mcp_proxy(authenticated_request(
            CloseMcpProxyRequest { session_id },
            &token,
        )),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(2), stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .finished
    );
    state
        .agents
        .daemon_tasks()
        .shutdown_runtime(Duration::from_secs(3))
        .await
        .unwrap();
    http.abort();
}
