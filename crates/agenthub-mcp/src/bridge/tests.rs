use std::{collections::BTreeMap, time::Duration};

use reqwest::header::HeaderMap;

use super::*;
use crate::{http::McpHttpTransport, protocol::ProtocolVersion};

mod batch;
mod lifecycle;
mod subscription;

fn session() -> Arc<McpProxySession> {
    session_with_budget(Arc::new(McpProxyBudget::default()))
}

fn session_with_budget(budget: Arc<McpProxyBudget>) -> Arc<McpProxySession> {
    let transport = McpHttpTransport::new(
        "http://127.0.0.1:1/mcp",
        HeaderMap::new(),
        Duration::from_secs(1),
    )
    .unwrap();
    let policy = McpBinding::new(
        "fixture".into(),
        &json!({"space":"fixture"}),
        &json!({"revision":1}),
        transport,
        BTreeMap::new(),
    )
    .unwrap();
    McpProxySession::new(
        "session".into(),
        Arc::new(McpProxyBinding::new(
            policy,
            Arc::new(|_, _, args| Ok(args)),
        )),
        budget,
    )
}

#[tokio::test]
async fn legacy_task_cancellation_requires_its_negotiated_capability() {
    for supports_cancel in [false, true] {
        let session = session();
        let mut capabilities = json!({"tasks":{"requests":{"tools":{"call":{}}}}});
        if supports_cancel {
            capabilities["tasks"]["cancel"] = json!({});
        }
        {
            let mut protocol = session.protocol.lock().await;
            protocol.begin(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
                "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"fixture","version":"1"}}})).unwrap();
            protocol.accept_initialize_response(&json!({"jsonrpc":"2.0","id":1,"result":{
                "protocolVersion":"2025-11-25","capabilities":capabilities,"serverInfo":{"name":"fixture","version":"1"}}}), None).unwrap();
            protocol
                .begin(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
                .unwrap();
        }
        session.apply_discovery(&mut json!({"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"tool","inputSchema":{"type":"object"}}]}}),
            &HttpContext { version: ProtocolVersion::November2025, session_id: None }, 0, &None).await.unwrap();
        let result = session
            .prepare(
                &executor(),
                json!({"jsonrpc":"2.0","id":3,"method":"tasks/cancel","params":{"taskId":"task"}}),
            )
            .await;
        assert_eq!(result.is_ok(), supports_cancel);
    }
}

#[tokio::test]
async fn shared_workspaces_bound_sessions_without_starving_initialize_callbacks() {
    let budget = Arc::new(McpProxyBudget::new(1, 1, 4096, 4096));
    let first = session_with_budget(budget.clone());
    let second = session_with_budget(budget);
    let initialize = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"fixture","version":"1"}}});
    let pending = first
        .prepare(&executor(), initialize.clone())
        .await
        .unwrap();
    assert!(matches!(
        second.prepare(&executor(), initialize.clone()).await,
        Err(McpPolicyError::Transport(McpTransportError::Capacity))
    ));
    first
        .observe(&json!({"jsonrpc":"2.0","id":"roots","method":"roots/list"}))
        .await
        .unwrap();
    let callback = tokio::time::timeout(
        Duration::from_secs(1),
        first.prepare(
            &executor(),
            json!({"jsonrpc":"2.0","id":"roots","result":{"roots":[]}}),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    drop((pending, callback));
    assert!(second.prepare(&executor(), initialize).await.is_ok());
}

#[tokio::test]
async fn retained_discovery_is_shared_and_refresh_reuses_its_charge() {
    let response = json!({"jsonrpc":"2.0","id":1,"result":{"tools":[{"name":"tool","inputSchema":{"type":"object"}}]}});
    let bytes = 2 * json_bytes(&response["result"]["tools"]).unwrap();
    let budget = Arc::new(McpProxyBudget::new(1, 1, 4096, bytes));
    let first = session_with_budget(budget.clone());
    let second = session_with_budget(budget.clone());
    let context = HttpContext {
        version: ProtocolVersion::November2025,
        session_id: None,
    };
    first
        .apply_discovery(&mut response.clone(), &context, 0, &None)
        .await
        .unwrap();
    assert_eq!(budget.retained.used(), bytes);
    first
        .apply_discovery(&mut response.clone(), &context, 0, &None)
        .await
        .unwrap();
    assert_eq!(
        second
            .apply_discovery(&mut response.clone(), &context, 0, &None)
            .await,
        Err(McpTransportError::Capacity)
    );
    assert!(second.discovery.lock().await.catalog.is_none());
    first
        .observe(&json!({"jsonrpc":"2.0","method":"notifications/tools/list_changed"}))
        .await
        .unwrap();
    assert_eq!(budget.retained.used(), 0);
    second
        .apply_discovery(&mut response.clone(), &context, 0, &None)
        .await
        .unwrap();
    drop(second);
    assert_eq!(budget.retained.used(), 0);
}

#[tokio::test]
async fn large_ids_remain_on_wire_without_large_retained_correlation_keys() {
    let session = session();
    let id = "large-id".repeat(16_384);
    let request = json!({"jsonrpc":"2.0","id":id,"method":"server/discover","params":{"_meta":{
        "io.modelcontextprotocol/protocolVersion":"2026-07-28",
        "io.modelcontextprotocol/clientInfo":{"name":"fixture","version":"1"},
        "io.modelcontextprotocol/clientCapabilities":{}}}});
    let prepared = session.prepare(&executor(), request.clone()).await.unwrap();
    assert_eq!(prepared.message["id"], id);
    drop(prepared);
    let keys = session.request_ids.lock().await;
    assert_eq!(keys.len(), 1);
    assert_eq!(std::mem::size_of_val(keys.iter().next().unwrap()), 32);
    drop(keys);
    assert!(matches!(
        session.prepare(&executor(), request).await,
        Err(McpPolicyError::Call)
    ));
}

fn executor() -> LoopReservation {
    LoopReservation {
        actor_id: "actor".into(),
        team_id: "team".into(),
        activation_id: Some("activation".into()),
        generation: 1,
        owner_id: "daemon".into(),
        lease_expires_at: 60,
        lease_seconds: 60,
        renewal_seconds: 10,
        session_id: Some("provider".into()),
        created_at: 0,
    }
}

#[tokio::test]
async fn duplicate_request_rejection_does_not_begin_initialization() {
    let session = session();
    let initialize = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"fixture","version":"1"}}});
    let prepared = session
        .prepare(&executor(), initialize.clone())
        .await
        .unwrap();
    drop(prepared);
    session.protocol.lock().await.initialization_failed();
    assert!(matches!(
        session.prepare(&executor(), initialize.clone()).await,
        Err(McpPolicyError::Call)
    ));
    let mut retry = initialize;
    retry["id"] = json!(2);
    assert!(session.prepare(&executor(), retry).await.is_ok());
}

#[tokio::test]
async fn stale_discovery_preserves_response_without_replacing_current_catalog() {
    let session = session();
    let context = HttpContext {
        version: ProtocolVersion::November2025,
        session_id: None,
    };
    let current = json!({"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"current","inputSchema":{"type":"object"}}],"nextCursor":"next"}});
    session.discovery.lock().await.generation = 2;
    session
        .apply_discovery(&mut current.clone(), &context, 2, &None)
        .await
        .unwrap();
    let stale = json!({"jsonrpc":"2.0","id":1,"result":{"tools":[{"name":"stale","inputSchema":{"type":"object"}}],"nextCursor":"old-next","vendor":{"preserved":true}}});
    for (generation, cursor) in [(1, None), (2, Some("old-cursor".into()))] {
        let mut response = stale.clone();
        session
            .apply_discovery(&mut response, &context, generation, &cursor)
            .await
            .unwrap();
        assert_eq!(response, stale);
        let discovery = session.discovery.lock().await;
        assert_eq!(
            discovery.catalog.as_ref().unwrap().advertised_tools(),
            current["result"]["tools"]
        );
        assert_eq!(discovery.next_cursor.as_deref(), Some("next"));
    }
}
