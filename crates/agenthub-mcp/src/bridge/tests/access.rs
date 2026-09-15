use super::*;
use crate::access::McpSelection;

fn restricted() -> Arc<McpProxySession> {
    session_with_access(
        Arc::new(McpProxyBudget::default()),
        McpAccessPolicy {
            tools: McpSelection::Names(["visible".to_owned()].into()),
            prompts: McpSelection::Names(["brief".to_owned()].into()),
            ..McpAccessPolicy::tools_only()
        },
    )
}

fn modern(id: i64, method: &str, mut params: Value) -> Value {
    params["_meta"] = json!({"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}});
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
}

#[tokio::test]
async fn access_rejection_precedes_request_ids_and_discovery_never_expands_tool_grants() {
    let session = restricted();
    assert!(matches!(
        session
            .prepare(
                &executor(),
                modern(1, "resources/read", json!({"uri":"mem://foreign"}))
            )
            .await,
        Err(McpPolicyError::Scope)
    ));
    assert!(session.request_ids.lock().await.is_empty());
    let visible = json!({"name":"visible","inputSchema":{"type":"object","properties":{"body":{"type":"string"}}},"extension":{"preserved":true}});
    let mut page = json!({"jsonrpc":"2.0","id":10,"result":{"tools":[visible,{"name":"hidden","inputSchema":{"type":"object"}}],"nextCursor":"next","extension":7}});
    session
        .apply_discovery(
            &mut page,
            &HttpContext {
                version: ProtocolVersion::July2026,
                session_id: None,
            },
            0,
            &None,
        )
        .await
        .unwrap();
    assert_eq!(page["result"]["tools"], json!([visible]));
    assert_eq!(page["result"]["nextCursor"], "next");
    assert_eq!(page["result"]["extension"], 7);
    assert!(matches!(
        session
            .prepare(
                &executor(),
                modern(1, "tools/call", json!({"name":"hidden","arguments":{}}))
            )
            .await,
        Err(McpPolicyError::Scope)
    ));
    assert!(session.request_ids.lock().await.is_empty());
    assert!(
        session
            .prepare(
                &executor(),
                modern(1, "tools/call", json!({"name":"visible","arguments":{}}))
            )
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn unauthorized_batch_member_cannot_advance_lifecycle_or_consume_other_ids() {
    let session = restricted();
    batch::awaiting_initialized(&session).await;
    let initialized = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
    let read =
        json!({"jsonrpc":"2.0","id":2,"method":"resources/read","params":{"uri":"mem://foreign"}});
    assert!(matches!(
        session
            .prepare(&executor(), json!([initialized, read]))
            .await,
        Err(McpPolicyError::Scope)
    ));
    assert!(session.protocol.lock().await.awaiting_initialized());
    assert!(session.request_ids.lock().await.is_empty());
    let prompt = json!({"jsonrpc":"2.0","id":2,"method":"prompts/get","params":{"name":"brief"}});
    assert!(
        session
            .prepare(&executor(), json!([initialized, prompt]))
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn unauthorized_callbacks_do_not_create_reply_authority_and_revoked_preparations_do_not_run()
{
    let session = restricted();
    assert!(
        session
            .observe(&json!([
                {"jsonrpc":"2.0","id":"allowed","method":"roots/list"},
                {"jsonrpc":"2.0","id":"foreign","method":"vendor/read_private"}
            ]))
            .await
            .is_err()
    );
    assert!(session.callbacks.lock().await.is_empty());
    let prepared = session
        .prepare(
            &executor(),
            modern(1, "prompts/get", json!({"name":"brief"})),
        )
        .await
        .unwrap();
    session.binding.revoke();
    let journal = JournaledMcpClient::new(
        agenthub_db::mcp_operations::McpOperationStore::new(
            sqlx::SqlitePool::connect_lazy("sqlite::memory:").unwrap(),
            agenthub_db::DaemonGeneration {
                node_id: "main".into(),
                generation: 1,
                owner_id: "daemon".into(),
                owner_pid: 1,
                claimed_at: 1,
            },
        ),
        ByteBudget::new(4096),
    );
    let (sender, mut receiver) = mpsc::channel(8);
    prepared.run(journal, sender).await;
    assert!(
        receiver.recv().await.is_none(),
        "revoked prepared requests must not try HTTP"
    );
}
