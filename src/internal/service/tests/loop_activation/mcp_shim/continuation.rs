use super::*;

pub(super) fn requested_input() -> Value {
    json!({"resultType":"input_required","requestState":"private-state-α\nopaque",
        "inputRequests":{"confirm":{"method":"elicitation/create","params":{"mode":"form","message":"Confirm write","requestedSchema":{"type":"object"}}}}})
}

pub(super) async fn respond(upstream: &Upstream, headers: &HeaderMap, message: &Value) -> Response {
    assert_eq!(headers["mcp-protocol-version"], "2026-07-28");
    assert!(headers.get("mcp-session-id").is_none());
    let result = if let Some(state) = message["params"].get("requestState") {
        assert_eq!(state, &requested_input()["requestState"]);
        assert_eq!(
            message["params"]["inputResponses"],
            json!({"confirm":{"action":"accept","content":{"approved":true}}})
        );
        let linked: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM mcp_operation_continuations c JOIN mcp_operation_attempts a \
            ON a.operation_id=c.operation_id AND a.number=c.attempt_number WHERE a.status='sent'",
        )
        .fetch_one(&upstream.db)
        .await
        .unwrap();
        assert_eq!(linked, 1, "continuation link must commit before HTTP");
        json!({"resultType":"complete","content":[{"type":"text","text":"private-mrtr-result"}],"extension":{"preserved":true}})
    } else {
        requested_input()
    };
    Json(json!({"jsonrpc":"2.0","id":message["id"],"result":result})).into_response()
}

#[tokio::test]
async fn real_mcp_shim_keeps_input_required_rounds_linked_without_replaying_the_write() {
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
    upstream.mrtr.store(true, Ordering::Release);
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
    let metadata = json!({"io.modelcontextprotocol/protocolVersion":"2026-07-28",
        "io.modelcontextprotocol/clientInfo":{"name":"mrtr-provider","version":"1"},
        "io.modelcontextprotocol/clientCapabilities":{"elicitation":{"form":{}}}});
    write_message(
        &mut input,
        &json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"_meta":metadata}}),
    )
    .await
    .unwrap();
    assert_eq!(
        next(&mut output).await["result"]["tools"][0]["name"],
        "write"
    );
    let first = json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"write","arguments":{"body":"private-mrtr-write"},"_meta":metadata}});
    write_message(&mut input, &first).await.unwrap();
    assert_eq!(
        next(&mut output).await,
        json!({"jsonrpc":"2.0","id":2,"result":requested_input()})
    );
    let mut follow = first.clone();
    follow["id"] = json!(3);
    follow["params"]["requestState"] = requested_input()["requestState"].clone();
    follow["params"]["inputResponses"] =
        json!({"confirm":{"action":"accept","content":{"approved":true}}});
    let mut invalid = follow.clone();
    invalid["params"]["arguments"]["space_id"] = "another-space".into();
    write_message(&mut input, &invalid).await.unwrap();
    assert!(next(&mut output).await.get("error").is_some());
    assert_eq!(upstream.calls.lock().unwrap().len(), 2);
    follow["id"] = json!(4);
    write_message(&mut input, &follow).await.unwrap();
    let response = next(&mut output).await;
    assert_eq!(response["id"], 4);
    assert_eq!(
        response["result"]["content"][0]["text"],
        "private-mrtr-result"
    );
    assert_eq!(response["result"]["extension"]["preserved"], true);
    let operations: Vec<String> = sqlx::query_scalar("SELECT id FROM mcp_operations")
        .fetch_all(&state.db)
        .await
        .unwrap();
    assert_eq!(operations.len(), 1);
    let attempts = journal
        .attempts(
            &reservation.team_id,
            &reservation.actor_id,
            &operations[0],
            0,
            100,
        )
        .await
        .unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(
        attempts[1]
            .continuation
            .as_ref()
            .unwrap()
            .parent_attempt_number,
        1
    );
    assert_eq!(
        attempts[1].status,
        agenthub_agent_domain::mcp_operations::McpOperationStatus::Succeeded
    );
    assert_eq!(upstream.calls.lock().unwrap().len(), 3);
    drop(input);
    assert!(
        tokio::time::timeout(Duration::from_secs(3), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
    let mut errors = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut errors)
        .await
        .unwrap();
    for value in ["private-state", "private-mrtr", "upstream-secret"] {
        assert!(!errors.contains(value));
    }
    state
        .agents
        .daemon_tasks()
        .shutdown_runtime(Duration::from_secs(3))
        .await
        .unwrap();
    grpc.abort();
    http.abort();
}
