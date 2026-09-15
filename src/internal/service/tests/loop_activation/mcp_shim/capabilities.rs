use super::*;
use agenthub_agent_domain::mcp_operations::McpOperationStatus;

async fn open(h: &Harness) -> (String, String) {
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
    (token, session)
}

#[tokio::test]
async fn mcp_undeclared_callback_cannot_hide_a_later_durable_write_success() {
    let h = setup().await;
    let (token, session) = open(&h).await;
    let mut initialize = access::exchange(&h, &token, &session, json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{"roots":{}},"clientInfo":{"name":"fixture","version":"1"}}})).await;
    let callback: Value =
        serde_json::from_str(&initialize.next().await.unwrap().unwrap().message_json).unwrap();
    let mut reply = access::exchange(
        &h,
        &token,
        &session,
        json!({"jsonrpc":"2.0","id":callback["id"],"result":{"roots":[]}}),
    )
    .await;
    assert!(reply.next().await.unwrap().unwrap().finished);
    assert!(initialize.next().await.unwrap().unwrap().finished);
    let mut initialized = access::exchange(
        &h,
        &token,
        &session,
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    )
    .await;
    assert!(initialized.next().await.unwrap().unwrap().finished);
    access::call(
        &h,
        &token,
        &session,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
    )
    .await;
    h.upstream
        .undeclared_callback
        .store(true, Ordering::Release);
    let stream = access::exchange(
        &h,
        &token,
        &session,
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{
        "name":"write","arguments":{"body":"capability-test"}}}),
    )
    .await;
    let frames = tokio::time::timeout(Duration::from_secs(5), stream.collect::<Vec<_>>())
        .await
        .unwrap();
    assert!(
        frames.iter().all(|frame| frame
            .as_ref()
            .map_or(true, |frame| frame.message_json.is_empty())),
        "neither the unsupported callback nor the final result can reach a closed provider session"
    );
    let events = h
        .journal
        .events(
            &h.run.team_id,
            &h.reservation.actor_id,
            h.reservation.activation_id.as_deref().unwrap(),
            0,
            100,
        )
        .await
        .unwrap();
    assert_eq!(events.last().unwrap().status, McpOperationStatus::Succeeded);
    assert_eq!(
        h.upstream
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| call["method"] == "tools/call")
            .count(),
        1
    );
    assert!(
        !h.state
            .agents
            .mcp_proxy()
            .unwrap()
            .session(&h.reservation, &session)
            .await
            .unwrap()
            .is_active()
    );
    h.state
        .agents
        .daemon_tasks()
        .shutdown_runtime(Duration::from_secs(3))
        .await
        .unwrap();
    h.http.abort();
}

fn modern(id: i64, method: &str, mut params: Value, elicitation: bool) -> Value {
    let mut capabilities = json!({"extensions":{"io.modelcontextprotocol/tasks":{}}});
    if elicitation {
        capabilities["elicitation"] = json!({"form":{}});
    }
    params["_meta"] = json!({"io.modelcontextprotocol/protocolVersion":"2026-07-28",
        "io.modelcontextprotocol/clientInfo":{"name":"fixture","version":"1"},
        "io.modelcontextprotocol/clientCapabilities":capabilities});
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
}

#[tokio::test]
async fn mcp_task_inputs_use_the_query_or_subscription_client_capabilities() {
    for subscription in [false, true] {
        let h = setup().await;
        h.upstream.tasks.store(true, Ordering::Release);
        let (token, session) = open(&h).await;
        access::call(
            &h,
            &token,
            &session,
            modern(1, "tools/list", json!({}), true),
        )
        .await;
        let created = access::call(
            &h,
            &token,
            &session,
            modern(
                2,
                "tools/call",
                json!({"name":"write","arguments":{"body":"task-capability"}}),
                true,
            ),
        )
        .await;
        assert_eq!(created["result"]["resultType"], "task");
        let (method, params) = if subscription {
            (
                "subscriptions/listen",
                json!({"notifications":{"taskIds":["private-task-handle"]}}),
            )
        } else {
            ("tasks/get", json!({"taskId":"private-task-handle"}))
        };
        // Supporting elicitation on task creation does not grant it to this later request.
        let mut stream =
            access::exchange(&h, &token, &session, modern(3, method, params, false)).await;
        if subscription {
            let ack: Value =
                serde_json::from_str(&stream.next().await.unwrap().unwrap().message_json).unwrap();
            assert_eq!(ack["method"], "notifications/subscriptions/acknowledged");
        }
        let frames = tokio::time::timeout(Duration::from_secs(5), stream.collect::<Vec<_>>())
            .await
            .unwrap();
        assert!(frames.iter().all(|frame| {
            frame
                .as_ref()
                .map_or(true, |frame| frame.message_json.is_empty())
        }));
        let inputs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mcp_operation_task_inputs")
            .fetch_one(&h.state.db)
            .await
            .unwrap();
        assert_eq!(
            inputs, 1,
            "observed task input facts persist before rejected delivery"
        );
        let events = h
            .journal
            .events(
                &h.run.team_id,
                &h.reservation.actor_id,
                h.reservation.activation_id.as_deref().unwrap(),
                0,
                100,
            )
            .await
            .unwrap();
        assert_eq!(
            events.last().unwrap().status,
            McpOperationStatus::OutcomeUnknown
        );
        h.state
            .agents
            .daemon_tasks()
            .shutdown_runtime(Duration::from_secs(3))
            .await
            .unwrap();
        h.http.abort();
    }
}
