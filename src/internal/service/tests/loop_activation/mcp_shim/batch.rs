use super::*;

pub(super) async fn handle(
    upstream: Arc<Upstream>,
    headers: HeaderMap,
    message: Value,
) -> Response {
    assert_eq!(headers["mcp-session-id"], "private-upstream-session");
    let members = message.as_array().unwrap();
    if members.iter().all(|member| member.get("method").is_none()) {
        assert_eq!(members.len(), 1);
        assert_eq!(members[0]["id"], "roots-1");
        assert_eq!(members[0]["result"]["roots"], json!([]));
        upstream.callback.notify_one();
        return StatusCode::ACCEPTED.into_response();
    }
    let tools = members
        .iter()
        .filter(|member| member["method"] == "tools/call")
        .count();
    let sent: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM mcp_operation_attempts WHERE status = 'sent'")
            .fetch_one(&upstream.db)
            .await
            .unwrap();
    assert_eq!(
        sent, tools as i64,
        "every batch write must be durable before HTTP"
    );
    let mut responses = Vec::new();
    for member in members {
        let result = match member["method"].as_str().unwrap() {
            "notifications/initialized" => {
                upstream.initialized.store(true, Ordering::Release);
                continue;
            }
            "notifications/progress" => continue,
            "tools/list" => {
                assert!(upstream.initialized.load(Ordering::Acquire));
                json!({"tools":[{"name":"write","inputSchema":{"type":"object","properties":{
                    "body":{"type":"string"},"space_id":{"type":"string"}}}}]})
            }
            "tools/call" => {
                assert_eq!(member["params"]["arguments"]["space_id"], "space-a");
                json!({"content":[{"type":"text","text":member["params"]["arguments"]["body"]}],"extension":{"preserved":true}})
            }
            "ping" => json!({}),
            _ => panic!("unexpected batch member"),
        };
        responses.push(json!({"jsonrpc":"2.0","id":member["id"],"result":result}));
    }
    if members
        .iter()
        .any(|member| member.pointer("/params/arguments/body") == Some(&json!("partial")))
    {
        return (
            [("content-type", "text/event-stream")],
            format!("data: {}\n\n", responses[0]),
        )
            .into_response();
    }
    if responses.is_empty() {
        return StatusCode::ACCEPTED.into_response();
    }
    responses.reverse();
    Json(Value::Array(responses)).into_response()
}

#[tokio::test]
async fn real_mcp_shim_preserves_march_batches_and_rejects_invalid_scope_before_http() {
    exercise(false).await;
}

#[tokio::test]
async fn real_mcp_shim_delivers_partial_batch_fact_before_closing_uncertain_exchange() {
    exercise(true).await;
}

async fn exercise(partial: bool) {
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
    } = setup().await;
    let token = signed_token(
        &authz,
        &reservation,
        &run.id,
        vec![InternalAction::McpProxy.as_str().into()],
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = format!("http://{}", listener.local_addr().unwrap());
    let incoming = futures::stream::unfold(listener, |listener| async move {
        Some((listener.accept().await.map(|(stream, _)| stream), listener))
    });
    let grpc = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(TeamInternalControlServer::new(service))
            .serve_with_incoming(incoming)
            .await
            .unwrap();
    });
    let file = LoopCredentialFile::create().unwrap();
    file.replace(&LoopCredentialEnvelope {
        actor_id: reservation.actor_id.clone(),
        run_id: run.id.clone(),
        activation_id: reservation.activation_id.clone().unwrap(),
        generation: reservation.generation,
        target,
        access_token: token,
        expires_at: chrono::Utc::now().timestamp() + 600,
        ca_cert_path: None,
    })
    .unwrap();
    let binary = crate::agenthub_binary::resolve_agenthub_binary_path()
        .expect("build the real binary first");
    let mut child = tokio::process::Command::new(binary)
        .args(["mcp-proxy", "--server-id", "fixture"])
        .env(LOOP_CREDENTIAL_FILE_ENV, &file.path)
        .env_remove("AGENTHUB_INTERNAL_GRPC_TOKEN")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    write_message(&mut input, &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-03-26","capabilities":{"roots":{}},"clientInfo":{"name":"batch-provider","version":"1"}}})).await.unwrap();
    let callback = next(&mut output).await;
    assert_eq!(callback[0]["method"], "roots/list");
    write_message(
        &mut input,
        &json!([{"jsonrpc":"2.0","id":callback[0]["id"],"result":{"roots":[]}}]),
    )
    .await
    .unwrap();
    assert_eq!(
        next(&mut output).await["result"]["protocolVersion"],
        "2025-03-26"
    );
    write_message(
        &mut input,
        &json!([
            {"jsonrpc":"2.0","method":"notifications/initialized"},
            {"jsonrpc":"2.0","id":2,"method":"tools/list"}
        ]),
    )
    .await
    .unwrap();
    assert_eq!(
        next(&mut output).await[0]["result"]["tools"][0]["name"],
        "write"
    );
    write_message(
        &mut input,
        &json!({"jsonrpc":"2.0","id":"json-batched-list","method":"tools/list"}),
    )
    .await
    .unwrap();
    assert_eq!(next(&mut output).await[0]["id"], "json-batched-list");
    write_message(
        &mut input,
        &json!({"jsonrpc":"2.0","id":"batched-list","method":"tools/list"}),
    )
    .await
    .unwrap();
    let listed = next(&mut output).await;
    assert_eq!(listed[0]["id"], "batched-list");
    assert_eq!(listed[0]["result"]["tools"][0]["name"], "write");
    assert_eq!(listed[1]["method"], "roots/list");
    write_message(
        &mut input,
        &json!([{"jsonrpc":"2.0","id":listed[1]["id"],"result":{"roots":[]}}]),
    )
    .await
    .unwrap();
    // Callback acknowledgment is empty and travels on an independent response stream.
    tokio::time::timeout(Duration::from_secs(8), upstream.callback.notified())
        .await
        .unwrap();
    write_message(
        &mut input,
        &json!([{"jsonrpc":"2.0","id":"after-callback","method":"ping"}]),
    )
    .await
    .unwrap();
    assert_eq!(next(&mut output).await[0]["id"], "after-callback");
    let write = |id, body| json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"write","arguments":{"body":body}}});
    let before = upstream.calls.lock().unwrap().len();
    let mut invalid = write(11, "wrong-scope");
    invalid["params"]["arguments"]["space_id"] = json!("other-space");
    write_message(&mut input, &json!([
        write(10, "must-not-send"), invalid,
        {"jsonrpc":"2.0","method":"notifications/progress","params":{"progressToken":"local","progress":1}}
    ])).await.unwrap();
    let denied = next(&mut output).await;
    assert_eq!(denied.as_array().unwrap().len(), 2);
    assert_eq!(denied[0]["id"], 10);
    assert_eq!(denied[1]["id"], 11);
    assert!(
        denied
            .as_array()
            .unwrap()
            .iter()
            .all(|member| member["error"]["code"] == -32000)
    );
    assert_eq!(upstream.calls.lock().unwrap().len(), before);
    let attempts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mcp_operation_attempts")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(attempts, 0);
    let batch = json!([write(3, if partial { "partial" } else { "first" }), write(4, "second"),
        {"jsonrpc":"2.0","id":5,"method":"ping"}]);
    write_message(&mut input, &batch).await.unwrap();
    let response = next(&mut output).await;
    if partial {
        assert_eq!(response["id"], 3);
        let status = tokio::time::timeout(Duration::from_secs(8), child.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(!status.success());
    } else {
        assert_eq!(
            response
                .as_array()
                .unwrap()
                .iter()
                .map(|member| member["id"].as_i64().unwrap())
                .collect::<Vec<_>>(),
            vec![5, 4, 3]
        );
        assert_eq!(response[1]["result"]["content"][0]["text"], "second");
        assert_eq!(response[2]["result"]["extension"]["preserved"], true);
    }
    let statuses: Vec<String> =
        sqlx::query_scalar("SELECT status FROM mcp_operation_attempts ORDER BY status")
            .fetch_all(&state.db)
            .await
            .unwrap();
    assert_eq!(
        statuses,
        if partial {
            vec!["outcome_unknown", "succeeded"]
        } else {
            vec!["succeeded", "succeeded"]
        }
    );
    assert_eq!(upstream.calls.lock().unwrap().len(), before + 1);
    assert_eq!(
        upstream.calls.lock().unwrap().last().unwrap()[0]["params"]["arguments"]["space_id"],
        "space-a"
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
    assert_eq!(events.len(), 6);
    drop(input);
    if !partial {
        assert!(
            tokio::time::timeout(Duration::from_secs(8), child.wait())
                .await
                .unwrap()
                .unwrap()
                .success()
        );
    }
    state
        .agents
        .daemon_tasks()
        .shutdown_runtime(Duration::from_secs(5))
        .await
        .unwrap();
    grpc.abort();
    http.abort();
}
