use std::{collections::BTreeMap, time::Duration};

use reqwest::header::HeaderMap;

use super::*;
use crate::{http::McpHttpTransport, protocol::ProtocolVersion};

fn session() -> Arc<McpProxySession> {
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
    )
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
