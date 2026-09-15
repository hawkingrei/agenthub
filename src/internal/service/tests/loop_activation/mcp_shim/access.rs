use super::*;
use crate::internal::proto::agenthub::internal::v1::ExchangeMcpProxyRequest;
use agenthub_mcp::access::{McpAccessPolicy, McpSelection};

pub(super) fn result(message: &Value) -> Value {
    match message["method"].as_str().unwrap() {
        "resources/list" => json!({"resources":[
            {"uri":"mem://a/item","name":"Allowed","extension":{"preserved":true}},
            {"uri":"mem://b/item","name":"Private"}],"nextCursor":"resource-page","extension":7}),
        "resources/templates/list" => json!({"resourceTemplates":[
            {"uriTemplate":"mem://a/{id}","name":"Allowed"},
            {"uriTemplate":"mem://b/{id}","name":"Private"}]}),
        "resources/read" => {
            let mut contents = json!([{"uri":message["params"]["uri"],"text":"allowed-resource","mimeType":"text/plain","extension":7}]);
            if message["id"] == 90 {
                contents
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"uri":"mem://b/item","text":"private-cross-scope-result"}));
            }
            json!({"contents":contents})
        }
        "prompts/list" => {
            json!({"prompts":[{"name":"brief","arguments":[{"name":"topic","required":true}],"extension":7},{"name":"private-prompt"}]})
        }
        "prompts/get" => {
            json!({"messages":[{"role":"user","content":{"type":"text","text":"allowed-prompt"}}],"extension":7})
        }
        "completion/complete" => {
            json!({"completion":{"values":["allowed"],"hasMore":false},"extension":7})
        }
        "resources/subscribe" | "resources/unsubscribe" => json!({}),
        _ => unreachable!(),
    }
}

pub(super) fn respond(message: &Value) -> Response {
    if message["method"] == "prompts/get" && matches!(message["id"].as_i64(), Some(91 | 92)) {
        let method = if message["id"] == 91 {
            "sampling/createMessage"
        } else {
            "roots/list"
        };
        return Json(json!({"jsonrpc":"2.0","id":message["id"],"result":{
            "resultType":"input_required","requestState":"opaque-prompt-state",
            "inputRequests":{"input":{"method":method,"params":{}}}
        }}))
        .into_response();
    }
    let mut result = result(message);
    if message.pointer("/params/_meta/io.modelcontextprotocol~1protocolVersion")
        == Some(&json!("2026-07-28"))
    {
        result["resultType"] = "complete".into();
    }
    Json(json!({"jsonrpc":"2.0","id":message["id"],"result":result})).into_response()
}

fn names(names: &[&str]) -> McpSelection {
    McpSelection::Names(names.iter().map(|value| (*value).to_owned()).collect())
}

fn request(id: i64, method: &str, mut params: Value) -> Value {
    params["_meta"] = json!({"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{"roots":{},"sampling":{}}});
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
}

async fn exchange(
    h: &Harness,
    token: &str,
    session: &str,
    message: Value,
) -> crate::internal::service::mcp_proxy::McpResponseStream {
    h.service
        .exchange_mcp_proxy(authenticated_request(
            ExchangeMcpProxyRequest {
                session_id: session.into(),
                message_json: message.to_string(),
            },
            token,
        ))
        .await
        .unwrap()
        .into_inner()
}

async fn call(h: &Harness, token: &str, session: &str, message: Value) -> Value {
    let mut stream = exchange(h, token, session, message).await;
    let frame = stream.next().await.unwrap().unwrap();
    assert!(frame.finished);
    serde_json::from_str(&frame.message_json).unwrap()
}

#[tokio::test]
async fn mcp_access_policy_scopes_rpc_discovery_calls_and_resource_subscriptions() {
    let access = McpAccessPolicy {
        tools: names(&["write"]),
        resources: names(&["mem://a/item"]),
        resource_templates: names(&["mem://a/{id}"]),
        prompts: names(&["brief"]),
        callbacks: names(&["roots/list"]),
        ..McpAccessPolicy::tools_only()
    };
    let h = setup_with_access(true, TrustedReplayPolicy::NonIdempotent, access).await;
    h.upstream.access_fixture.store(true, Ordering::Release);
    let token = signed_token(
        &h.authz,
        &h.reservation,
        &h.run.id,
        vec![InternalAction::McpProxy.as_str().into()],
    );
    let session = h
        .service
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
    let discovery = call(
        &h,
        &token,
        &session,
        request(1, "server/discover", json!({})),
    )
    .await;
    assert!(discovery["result"]["capabilities"]["resources"].is_object());
    assert!(discovery["result"]["capabilities"].get("logging").is_none());
    assert_eq!(
        discovery["result"]["extension"],
        json!({"preserved":[1,true,null]})
    );
    let first = call(&h, &token, &session, request(2, "tools/list", json!({}))).await;
    assert_eq!(first["result"]["tools"][0]["name"], "write");
    assert_eq!(first["result"]["nextCursor"], "page-2");
    let second = call(
        &h,
        &token,
        &session,
        request(3, "tools/list", json!({"cursor":"page-2"})),
    )
    .await;
    assert_eq!(second["result"]["tools"], json!([]));
    let calls = h.upstream.calls.lock().unwrap().len();
    for (method, params) in [
        ("tools/call", json!({"name":"read","arguments":{}})),
        ("resources/read", json!({"uri":"mem://b/item"})),
        ("resources/subscribe", json!({"uri":"mem://b/item"})),
        ("resources/unsubscribe", json!({"uri":"mem://b/item"})),
        ("prompts/get", json!({"name":"private-prompt"})),
        (
            "completion/complete",
            json!({"ref":{"type":"ref/resource","uri":"mem://b/{id}"},"argument":{"name":"id","value":"x"}}),
        ),
        ("logging/setLevel", json!({"level":"debug"})),
        (
            "subscriptions/listen",
            json!({"notifications":{"resourceSubscriptions":["mem://a/item","mem://b/item"]}}),
        ),
    ] {
        let response = call(&h, &token, &session, request(10, method, params)).await;
        assert!(response.get("error").is_some(), "{method}");
        assert_eq!(h.upstream.calls.lock().unwrap().len(), calls);
    }
    // Denied requests did not consume this ID or dispatch any server work.
    let resources = call(
        &h,
        &token,
        &session,
        request(10, "resources/list", json!({})),
    )
    .await;
    assert_eq!(
        resources["result"]["resources"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        resources["result"]["resources"][0]["extension"]["preserved"],
        true
    );
    assert_eq!(resources["result"]["nextCursor"], "resource-page");
    let templates = call(
        &h,
        &token,
        &session,
        request(11, "resources/templates/list", json!({})),
    )
    .await;
    assert_eq!(
        templates["result"]["resourceTemplates"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let prompts = call(&h, &token, &session, request(12, "prompts/list", json!({}))).await;
    assert_eq!(
        prompts["result"]["prompts"],
        json!([{"name":"brief","arguments":[{"name":"topic","required":true}],"extension":7}])
    );
    let prompt = call(
        &h,
        &token,
        &session,
        request(13, "prompts/get", json!({"name":"brief"})),
    )
    .await;
    assert_eq!(
        prompt["result"]["messages"][0]["content"]["text"],
        "allowed-prompt"
    );
    let resource = call(
        &h,
        &token,
        &session,
        request(14, "resources/read", json!({"uri":"mem://a/item"})),
    )
    .await;
    assert_eq!(
        resource["result"]["contents"][0]["text"],
        "allowed-resource"
    );
    assert_eq!(resource["result"]["contents"][0]["extension"], 7);
    let completion = call(&h,&token,&session,request(15,"completion/complete",json!({"ref":{"type":"ref/resource","uri":"mem://a/{id}"},"argument":{"name":"id","value":"x"}}))).await;
    assert_eq!(
        completion["result"]["completion"]["values"],
        json!(["allowed"])
    );
    let mut subscribed = exchange(
        &h,
        &token,
        &session,
        request(
            16,
            "subscriptions/listen",
            json!({"notifications":{"resourceSubscriptions":["mem://a/item"]}}),
        ),
    )
    .await;
    let acknowledged: Value =
        serde_json::from_str(&subscribed.next().await.unwrap().unwrap().message_json).unwrap();
    assert_eq!(
        acknowledged["method"],
        "notifications/subscriptions/acknowledged"
    );
    let bad = call(
        &h,
        &token,
        &session,
        request(90, "resources/read", json!({"uri":"mem://a/item"})),
    )
    .await;
    assert!(bad.get("error").is_some());
    assert!(!bad.to_string().contains("private-cross-scope-result"));
    let forbidden_input = call(
        &h,
        &token,
        &session,
        request(91, "prompts/get", json!({"name":"brief"})),
    )
    .await;
    assert_eq!(forbidden_input["error"]["code"], -32000);
    assert!(forbidden_input.get("result").is_none());
    let permitted_input = call(
        &h,
        &token,
        &session,
        request(92, "prompts/get", json!({"name":"brief"})),
    )
    .await;
    assert_eq!(
        permitted_input["result"]["requestState"],
        "opaque-prompt-state"
    );
    assert_eq!(
        permitted_input["result"]["inputRequests"]["input"]["method"],
        "roots/list"
    );
    let calls = h.upstream.calls.lock().unwrap().len();
    let mut continuation = request(
        93,
        "prompts/get",
        json!({"name":"brief",
        "requestState":"opaque-prompt-state","inputResponses":{"input":{"roots":[]}},"vendor":"changed"}),
    );
    let changed = call(&h, &token, &session, continuation.clone()).await;
    assert!(changed.get("error").is_some());
    assert_eq!(h.upstream.calls.lock().unwrap().len(), calls);
    continuation["id"] = json!(94);
    continuation["params"]
        .as_object_mut()
        .unwrap()
        .remove("vendor");
    let completed = call(&h, &token, &session, continuation.clone()).await;
    assert_eq!(
        completed["result"]["messages"][0]["content"]["text"],
        "allowed-prompt"
    );
    assert_eq!(h.upstream.calls.lock().unwrap().len(), calls + 1);
    assert_eq!(h.upstream.calls.lock().unwrap().last(), Some(&continuation));
    continuation["id"] = json!(95);
    let repeated = call(&h, &token, &session, continuation).await;
    assert!(repeated.get("error").is_some());
    assert_eq!(h.upstream.calls.lock().unwrap().len(), calls + 1);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mcp_operations")
        .fetch_one(&h.state.db)
        .await
        .unwrap();
    assert_eq!(count, 0, "non-tool authorization cannot invent a tool send");
    let still_active = call(
        &h,
        &token,
        &session,
        request(99, "resources/read", json!({"uri":"mem://a/item"})),
    )
    .await;
    assert_eq!(
        still_active["result"]["contents"][0]["text"],
        "allowed-resource"
    );
    h.binding.revoke();
    let calls = h.upstream.calls.lock().unwrap().len();
    let revoked = call(
        &h,
        &token,
        &session,
        request(100, "resources/read", json!({"uri":"mem://a/item"})),
    )
    .await;
    assert_eq!(revoked["id"], 100);
    assert_eq!(revoked["error"]["code"], -32000);
    assert!(revoked.get("result").is_none());
    assert_eq!(h.upstream.calls.lock().unwrap().len(), calls);
    drop(subscribed);
    h.state
        .agents
        .daemon_tasks()
        .shutdown_runtime(Duration::from_secs(3))
        .await
        .unwrap();
    h.http.abort();
}

#[tokio::test]
async fn mcp_access_policy_applies_to_every_march_batch_discovery_result() {
    let h = setup_with_access(
        true,
        TrustedReplayPolicy::NonIdempotent,
        McpAccessPolicy {
            resources: names(&["mem://a/item"]),
            prompts: names(&["brief"]),
            ..McpAccessPolicy::tools_only()
        },
    )
    .await;
    h.upstream.access_fixture.store(true, Ordering::Release);
    let token = signed_token(
        &h.authz,
        &h.reservation,
        &h.run.id,
        vec![InternalAction::McpProxy.as_str().into()],
    );
    let session = h
        .service
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
    let mut init = exchange(&h, &token, &session, json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-03-26","capabilities":{"roots":{}},"clientInfo":{"name":"fixture","version":"1"}}})).await;
    let callbacks: Value =
        serde_json::from_str(&init.next().await.unwrap().unwrap().message_json).unwrap();
    let mut reply = exchange(
        &h,
        &token,
        &session,
        json!([{"jsonrpc":"2.0","id":callbacks[0]["id"],"result":{"roots":[]}}]),
    )
    .await;
    assert!(reply.next().await.unwrap().unwrap().finished);
    assert!(init.next().await.unwrap().unwrap().finished);
    let initialized = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
    let denied = call(&h,&token,&session,json!([initialized,{"jsonrpc":"2.0","id":2,"method":"resources/read","params":{"uri":"mem://b/item"}}])).await;
    assert!(denied[0].get("error").is_some());
    assert!(!h.upstream.initialized.load(Ordering::Acquire));
    let calls = h.upstream.calls.lock().unwrap().len();
    let mut stream = exchange(
        &h,
        &token,
        &session,
        json!([initialized,
        {"jsonrpc":"2.0","id":2,"method":"resources/list"},
        {"jsonrpc":"2.0","id":3,"method":"prompts/list"}]),
    )
    .await;
    let response: Value =
        serde_json::from_str(&stream.next().await.unwrap().unwrap().message_json).unwrap();
    assert_eq!(response[0]["id"], 3);
    assert_eq!(
        response[0]["result"]["prompts"].as_array().unwrap().len(),
        1
    );
    assert_eq!(response[0]["result"]["prompts"][0]["extension"], 7);
    assert_eq!(response[1]["id"], 2);
    assert_eq!(
        response[1]["result"]["resources"].as_array().unwrap().len(),
        1
    );
    assert_eq!(response[1]["result"]["nextCursor"], "resource-page");
    assert!(stream.next().await.unwrap().unwrap().finished);
    assert_eq!(h.upstream.calls.lock().unwrap().len(), calls + 1);
    h.state
        .agents
        .daemon_tasks()
        .shutdown_runtime(Duration::from_secs(3))
        .await
        .unwrap();
    h.http.abort();
}
