use agenthub_agent_domain::loop_runtime::LoopLaunchSnapshot;

use super::*;
use crate::internal::proto::agenthub::internal::v1::ExchangeMcpProxyRequest;

#[tokio::test]
async fn mcp_bootstrap_completes_legacy_callbacks_before_running_but_cannot_write() {
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
    } = setup_with_running(false).await;
    let store = LoopStore::new(state.db.clone());
    let token = signed_token(
        &authz,
        &reservation,
        &run.id,
        vec![InternalAction::McpProxy.as_str().into()],
    );
    let open = || {
        service.open_mcp_proxy(authenticated_request(
            OpenMcpProxyRequest {
                server_id: "fixture".into(),
            },
            &token,
        ))
    };
    assert_eq!(
        open().await.unwrap_err().code(),
        tonic::Code::PermissionDenied
    );
    store
        .record_launch(
            &reservation,
            &LoopLaunchSnapshot {
                version: 1,
                provider_id: "fake-acp".into(),
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
    let session_id = open().await.unwrap().into_inner().session_id;
    let exchange = |message: Value| {
        service.exchange_mcp_proxy(authenticated_request(
            ExchangeMcpProxyRequest {
                session_id: session_id.clone(),
                message_json: message.to_string(),
            },
            &token,
        ))
    };
    let initialize = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{"roots":{}},"clientInfo":{"name":"startup-provider","version":"1"}}});
    let mut response = exchange(initialize).await.unwrap().into_inner();
    let callback = response.next().await.unwrap().unwrap();
    assert!(!callback.finished);
    let callback: Value = serde_json::from_str(&callback.message_json).unwrap();
    assert_eq!(callback["method"], "roots/list");
    let mut reply = exchange(json!({"jsonrpc":"2.0","id":callback["id"],"result":{"roots":[]}}))
        .await
        .unwrap()
        .into_inner();
    let ack = reply.next().await.unwrap().unwrap();
    assert!(ack.finished && ack.message_json.is_empty());
    let initialized = response.next().await.unwrap().unwrap();
    assert!(initialized.finished);
    assert_eq!(
        serde_json::from_str::<Value>(&initialized.message_json).unwrap()["result"]["protocolVersion"],
        "2025-11-25"
    );
    let mut ready = exchange(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .await
        .unwrap()
        .into_inner();
    assert!(ready.next().await.unwrap().unwrap().finished);
    let mut discovery = exchange(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}))
        .await
        .unwrap()
        .into_inner();
    let page = discovery.next().await.unwrap().unwrap();
    assert!(page.finished);
    assert_eq!(
        serde_json::from_str::<Value>(&page.message_json).unwrap()["result"]["tools"][0]["name"],
        "write"
    );
    let write = json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"write","arguments":{"body":"startup-write"}}});
    let calls_before = upstream.calls.lock().unwrap().len();
    for message in [
        write.clone(),
        json!({"jsonrpc":"2.0","id":4,"method":"resources/read","params":{"uri":"private://resource"}}),
        json!({"jsonrpc":"2.0","id":5,"method":"prompts/get","params":{"name":"private"}}),
        json!({"jsonrpc":"2.0","id":6,"method":"tasks/get","params":{"taskId":"task"}}),
        json!([write.clone()]),
    ] {
        assert_eq!(
            exchange(message).await.err().unwrap().code(),
            tonic::Code::PermissionDenied
        );
    }
    assert_eq!(upstream.calls.lock().unwrap().len(), calls_before);
    assert!(
        journal
            .events(
                &run.team_id,
                &reservation.actor_id,
                reservation.activation_id.as_deref().unwrap(),
                0,
                100
            )
            .await
            .unwrap()
            .is_empty()
    );
    let metadata = authenticated_request((), &token);
    assert!(
        service
            .authenticate_execution(metadata.metadata(), false)
            .await
            .is_err()
    );
    store
        .mark_running(&reservation, chrono::Utc::now().timestamp())
        .await
        .unwrap();
    assert!(
        service
            .authenticate_execution(metadata.metadata(), false)
            .await
            .is_ok()
    );
    let mut result = exchange(write).await.unwrap().into_inner();
    let mut terminal = None;
    while let Some(frame) = result.next().await {
        let frame = frame.unwrap();
        if frame.finished {
            terminal = Some(serde_json::from_str::<Value>(&frame.message_json).unwrap());
            break;
        }
    }
    assert_eq!(
        terminal.unwrap()["result"]["content"][0]["text"],
        "private-tool-result"
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
    http.abort();
}
