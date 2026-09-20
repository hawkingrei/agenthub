use agenthub_agent_domain::loop_runtime::LoopLaunchSnapshot;

use super::*;
use crate::internal::proto::agenthub::internal::v1::ExchangeMcpProxyRequest;

pub(super) fn request(id: &str, client: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"server/discover","params":{"_meta":{
        "io.modelcontextprotocol/protocolVersion":"2026-07-28",
        "io.modelcontextprotocol/clientInfo":{"name":client,"version":"1"},
        "io.modelcontextprotocol/clientCapabilities":{}
    }}})
}

pub(super) fn result() -> Value {
    json!({"resultType":"complete","supportedVersions":["2026-07-28"],
        "capabilities":{"tools":{},"experimental":{"vendor":{"preserved":true}}},
        "_meta":{"io.modelcontextprotocol/serverInfo":{"name":"fixture","version":"1"}},
        "instructions":"Fixture guidance","ttlMs":3600000,"cacheScope":"private",
        "extension":{"preserved":[1,true,null]}})
}

#[tokio::test]
async fn modern_mcp_discovery_preserves_metadata_and_cache_hints_during_startup() {
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
    LoopStore::new(state.db.clone())
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
    let discovery = request("discovery", "modern-client");
    let mut stream = service
        .exchange_mcp_proxy(authenticated_request(
            ExchangeMcpProxyRequest {
                session_id: session_id.clone(),
                message_json: discovery.to_string(),
            },
            &token,
        ))
        .await
        .unwrap()
        .into_inner();
    let frame = stream.next().await.unwrap().unwrap();
    assert!(frame.finished);
    assert_eq!(
        serde_json::from_str::<Value>(&frame.message_json).unwrap(),
        json!({"jsonrpc":"2.0","id":"discovery","result":result()})
    );
    assert!(stream.next().await.is_none());
    assert_eq!(*upstream.calls.lock().unwrap(), vec![discovery.clone()]);

    // Discovery conveys upstream data; it must not admit tools before the executor is running.
    let mut call = discovery;
    call["id"] = json!("write");
    call["method"] = json!("tools/call");
    call["params"]["name"] = json!("write");
    call["params"]["arguments"] = json!({"body":"not-sent"});
    let denied = service
        .exchange_mcp_proxy(authenticated_request(
            ExchangeMcpProxyRequest {
                session_id,
                message_json: call.to_string(),
            },
            &token,
        ))
        .await
        .err()
        .unwrap();
    assert_eq!(denied.code(), Code::PermissionDenied);
    assert_eq!(upstream.calls.lock().unwrap().len(), 1);
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
    state
        .agents
        .daemon_tasks()
        .shutdown_runtime(Duration::from_secs(5))
        .await
        .unwrap();
    http.abort();
}
