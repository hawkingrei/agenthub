use super::*;
use crate::access::McpSelection;
use agenthub_agent_domain::app_tools::AppManifest;

fn pinned_session() -> Arc<McpProxySession> {
    let manifest: AppManifest = serde_json::from_value(json!({
        "schema_version":1,"scopes":["read"],"tools":[{
            "name":"lookup","input_schema":{"type":"object","properties":{"key":{"type":"string"}},"required":["key"]},
            "output_schema":{"type":"object","properties":{"found":{"type":"boolean"}},"required":["found"]},
            "required_scopes":["read"],"replay":{"kind":"read_only"}
        }]
    })).unwrap();
    let manifest = Arc::new(manifest.compile().unwrap());
    let scopes = ["read".to_owned()].into();
    let access = McpAccessPolicy {
        tools: McpSelection::Names(manifest.allowed_tools(&scopes).unwrap()),
        ..Default::default()
    };
    let transport = McpHttpTransport::new(
        "http://127.0.0.1:1/mcp",
        HeaderMap::new(),
        Duration::from_secs(1),
    )
    .unwrap();
    let policy = McpBinding::new(
        "app-fixture".into(),
        &json!({"namespace":"fixture"}),
        &json!({"version":1}),
        transport,
        BTreeMap::new(),
    )
    .unwrap();
    let arguments = manifest.clone();
    let argument_scopes = scopes.clone();
    let binding = McpProxyBinding::new(
        policy,
        access,
        Arc::new(move |name, schema, args| {
            arguments
                .validate_arguments(name, schema, &args, &argument_scopes)
                .map_err(|_| McpPolicyError::Call)?;
            Ok(args)
        }),
    )
    .with_tool_declaration_validator(Arc::new(move |declaration| {
        manifest
            .validate_declaration(declaration, &scopes)
            .map_err(|_| McpPolicyError::Catalog)
    }));
    McpProxySession::new(
        "session".into(),
        Arc::new(binding),
        Arc::new(McpProxyBudget::default()),
    )
}

fn declaration() -> Value {
    json!({"name":"lookup","inputSchema":{"type":"object","properties":{"key":{"type":"string"}},"required":["key"]},
        "outputSchema":{"type":"object","properties":{"found":{"type":"boolean"}},"required":["found"]},"extension":{"preserved":true}})
}

fn context() -> HttpContext {
    HttpContext {
        version: ProtocolVersion::July2026,
        session_id: None,
    }
}

fn call(id: i64, name: &str, arguments: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":arguments,
        "_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}})
}

#[tokio::test]
async fn manifest_filters_unapproved_tools_and_validates_native_arguments() {
    let session = pinned_session();
    let mut page = json!({"jsonrpc":"2.0","id":1,"result":{"tools":[declaration(),{"name":"undeclared","inputSchema":{"type":"object"}}],"nextCursor":"more"}});
    session
        .apply_discovery(&mut page, &context(), 0, &None)
        .await
        .unwrap();
    assert_eq!(page["result"]["tools"], json!([declaration()]));
    assert_eq!(page["result"]["nextCursor"], "more");
    assert!(
        session
            .prepare(&executor(), call(2, "undeclared", json!({})))
            .await
            .is_err()
    );
    assert!(
        session
            .prepare(&executor(), call(3, "lookup", json!({"key":1})))
            .await
            .is_err()
    );
    let message = call(4, "lookup", json!({"key":"native-value"}));
    let prepared = session.prepare(&executor(), message.clone()).await.unwrap();
    assert_eq!(prepared.message, message);
    assert!(
        session
            .observe(&json!({"jsonrpc":"2.0","id":"callback","method":"roots/list"}))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn manifest_schema_drift_closes_session_even_after_a_valid_catalog() {
    for (field, schema) in [
        ("inputSchema", json!({"type":"object"})),
        ("outputSchema", json!({"type":"object"})),
        ("inputSchema", Value::Null),
        (
            "inputSchema",
            json!({"type":"object","properties":{"key":{"type":"string","x-mcp-header":"invalid header"}},"required":["key"]}),
        ),
    ] {
        let session = pinned_session();
        let mut page =
            json!({"jsonrpc":"2.0","id":1,"result":{"tools":[declaration()],"nextCursor":"more"}});
        session
            .apply_discovery(&mut page, &context(), 0, &None)
            .await
            .unwrap();
        assert!(session.discovery.lock().await.catalog.is_some());
        let mut changed = declaration();
        changed[field] = schema;
        let mut next = json!({"jsonrpc":"2.0","id":2,"result":{"tools":[changed]}});
        assert_eq!(
            session
                .apply_discovery(&mut next, &context(), 0, &Some("more".into()))
                .await,
            Err(McpTransportError::InvalidResponse)
        );
        assert!(!session.is_active());
        assert!(
            session
                .prepare(&executor(), call(3, "lookup", json!({"key":"valid"})))
                .await
                .is_err()
        );
    }
}
