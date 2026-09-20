use std::{collections::BTreeMap, sync::Arc, time::Duration};

use agenthub_agent_domain::mcp_operations::{McpOperationStatus, McpReplaySafety};
use agenthub_mcp::{
    http::{HttpContext, McpHttpTransport},
    journal::JournaledMcpClient,
    policy::{McpBinding, McpCallContext, McpToolCatalog, TrustedReplayPolicy},
    protocol::ProtocolVersion,
};
use tokio::sync::{Notify, mpsc};

use super::*;

#[tokio::test]
async fn mcp_send_remains_daemon_owned_after_authenticated_request_disconnects() {
    let (state, service, authz, run, reservation) = fixture().await;
    agenthub_db::mcp_operations::migrate_mcp_operations(&state.db)
        .await
        .unwrap();
    let directory = std::env::temp_dir().join(format!("agenthub-mcp-daemon-{}", Uuid::new_v4()));
    let mut daemon = crate::daemon_instance::DaemonInstanceGuard::acquire(
        &directory.join("control.sqlite"),
        "main",
    )
    .unwrap();
    daemon.claim_generation(&state.db).await.unwrap();
    let journal = daemon.mcp_operation_store(&state.db).unwrap();
    let received = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/mcp", listener.local_addr().unwrap());
    let router = axum::Router::new().route("/mcp", axum::routing::post({
        let db = state.db.clone();
        let received = received.clone();
        let release = release.clone();
        move |axum::Json(request): axum::Json<Value>| {
            let db = db.clone();
            let received = received.clone();
            let release = release.clone();
            async move {
                let sent: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mcp_operation_attempts WHERE status = 'sent'").fetch_one(&db).await.unwrap();
                assert_eq!(sent, 1);
                received.notify_one();
                release.notified().await;
                axum::Json(json!({"jsonrpc":"2.0","id":request["id"],"result":{"content":[{"type":"text","text":"late-private-result"}]}}))
            }
        }
    }));
    let upstream = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let binding = McpBinding::new(
        "fixture".into(),
        &json!({"service":"fake","space":"team"}),
        &json!({"revision":1}),
        McpHttpTransport::new(
            &endpoint,
            reqwest::header::HeaderMap::new(),
            Duration::from_secs(3),
        )
        .unwrap(),
        BTreeMap::from([("write".into(), TrustedReplayPolicy::NonIdempotent)]),
    )
    .unwrap();
    let catalog = McpToolCatalog::from_tools(&json!([{"name":"write","inputSchema":{"type":"object","properties":{"value":{"type":"string"}}}}]), ProtocolVersion::November2025).unwrap();
    let call = binding.prepare_call(&catalog, &McpCallContext {
        executor:&reservation, proxy_session_id:"fixture-session", http:&HttpContext {version:ProtocolVersion::November2025,session_id:None},
    }, json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"write","arguments":{"value":"private-value"}}}), |_, args| Ok(args)).unwrap();
    let request =
        authenticated_request(request(), &token(&authz, "reviewer", &run.id, &reservation));
    let metadata = request.metadata().clone();
    let task_metadata = metadata.clone();
    let task_service = service.clone();
    let client = JournaledMcpClient::new(
        journal.clone(),
        agenthub_mcp::budget::ByteBudget::new(8 * agenthub_mcp::MAX_MESSAGE_BYTES),
    );
    let (events, receiver) = mpsc::channel(2);
    let caller = tokio::spawn(async move {
        service
            .complete_control_request(&metadata, "test_mcp_control", async move {
                let (_principal, _guard) = task_service
                    .authenticate_execution(&task_metadata, false)
                    .await?;
                let result = client
                    .run(call, events)
                    .await
                    .map_err(|_| tonic::Status::unavailable("MCP call failed"))?;
                Ok(tonic::Response::new(result))
            })
            .await
    });
    tokio::time::timeout(Duration::from_secs(3), received.notified())
        .await
        .unwrap();
    caller.abort();
    assert!(matches!(caller.await, Err(error) if error.is_cancelled()));
    drop(receiver);
    let gate = state.agents.loop_operation_gate("reviewer").await;
    assert!(
        gate.try_write().is_err(),
        "disconnect released the operation guard before send settled"
    );
    release.notify_one();
    state
        .agents
        .daemon_tasks()
        .shutdown_runtime(Duration::from_secs(3))
        .await
        .unwrap();
    assert!(gate.try_write().is_ok());
    let events = journal
        .events(
            &run.team_id,
            "reviewer",
            reservation.activation_id.as_deref().unwrap(),
            0,
            100,
        )
        .await
        .unwrap();
    assert_eq!(
        events.iter().map(|event| event.status).collect::<Vec<_>>(),
        [
            McpOperationStatus::Prepared,
            McpOperationStatus::Sent,
            McpOperationStatus::Succeeded
        ]
    );
    let record = journal
        .operation(&run.team_id, "reviewer", &events[0].operation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.intent.replay_safety, McpReplaySafety::NonIdempotent);
    assert!(!serde_json::to_string(&record).unwrap().contains("private"));
    upstream.abort();
    drop(daemon);
    std::fs::remove_dir_all(directory).unwrap();
}
