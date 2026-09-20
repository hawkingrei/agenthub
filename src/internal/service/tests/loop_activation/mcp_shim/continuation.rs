use super::*;

pub(super) fn requested_input() -> Value {
    json!({"resultType":"input_required","requestState":"private-state-α\nopaque",
        "inputRequests":{"confirm":{"method":"elicitation/create","params":{"mode":"form","message":"Confirm write","requestedSchema":{"type":"object"}}}}})
}

pub(super) async fn respond(upstream: &Upstream, headers: &HeaderMap, message: &Value) -> Response {
    assert_eq!(headers["mcp-protocol-version"], "2026-07-28");
    assert!(headers.get("mcp-session-id").is_none());
    assert_eq!(
        message["params"]["arguments"]["request_id"],
        "caller-stable-id"
    );
    let result = if let Some(state) = message["params"].get("requestState") {
        assert_eq!(state, &requested_input()["requestState"]);
        assert_eq!(
            message["params"]["inputResponses"],
            json!({"confirm":{"action":"accept","content":{"approved":true}}})
        );
        let linked: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM mcp_operation_attempts a \
            LEFT JOIN mcp_operation_continuation_retries r ON r.operation_id=a.operation_id AND r.attempt_number=a.number \
            JOIN mcp_operation_continuations c ON c.operation_id=a.operation_id \
                AND c.attempt_number=COALESCE(r.continuation_attempt_number,a.number) WHERE a.status='sent'",
        )
        .fetch_one(&upstream.db)
        .await
        .unwrap();
        assert_eq!(linked, 1, "continuation link must commit before HTTP");
        if upstream.mrtr_drop_response.swap(false, Ordering::AcqRel) {
            return axum::http::Response::builder()
                .header("content-type", "application/json")
                .body(axum::body::Body::from("{\"jsonrpc\":\"2.0\",\"result\":"))
                .unwrap();
        }
        json!({"resultType":"complete","content":[{"type":"text","text":"private-mrtr-result"}],"extension":{"preserved":true}})
    } else {
        requested_input()
    };
    Json(json!({"jsonrpc":"2.0","id":message["id"],"result":result})).into_response()
}

#[tokio::test]
async fn real_mcp_shim_keeps_input_required_rounds_linked_without_replaying_the_write() {
    exercise_continuation(false).await;
}

#[tokio::test]
async fn real_mcp_shim_retries_only_the_declared_unchanged_continuation_round() {
    exercise_continuation(true).await;
}

async fn exercise_continuation(retry: bool) {
    let replay = if retry {
        TrustedReplayPolicy::StableIdentity {
            property_path: vec!["request_id".into()],
        }
    } else {
        TrustedReplayPolicy::NonIdempotent
    };
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
    } = setup_with_replay(true, replay).await;
    upstream.mrtr.store(true, Ordering::Release);
    upstream.mrtr_drop_response.store(retry, Ordering::Release);
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
    let first = json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"write","arguments":{"body":"private-mrtr-write","request_id":"caller-stable-id"},"_meta":metadata}});
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
    if retry {
        assert!(next(&mut output).await.get("error").is_some());
        assert_eq!(upstream.calls.lock().unwrap().len(), 3);
        let mut changed = follow.clone();
        changed["id"] = json!(5);
        changed["params"]["inputResponses"]["confirm"]["action"] = "cancel".into();
        write_message(&mut input, &changed).await.unwrap();
        assert!(next(&mut output).await.get("error").is_some());
        assert_eq!(upstream.calls.lock().unwrap().len(), 3);
        follow["id"] = json!(6);
        write_message(&mut input, &follow).await.unwrap();
    }
    let response = next(&mut output).await;
    assert_eq!(response["id"], follow["id"]);
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
    assert_eq!(attempts.len(), if retry { 3 } else { 2 });
    assert_eq!(
        attempts[1]
            .continuation
            .as_ref()
            .unwrap()
            .parent_attempt_number,
        1
    );
    assert_eq!(
        attempts.last().unwrap().status,
        agenthub_agent_domain::mcp_operations::McpOperationStatus::Succeeded
    );
    assert_eq!(
        upstream.calls.lock().unwrap().len(),
        if retry { 4 } else { 3 }
    );
    if retry {
        assert_eq!(
            attempts[1].status,
            agenthub_agent_domain::mcp_operations::McpOperationStatus::OutcomeUnknown
        );
        let link = attempts[2].continuation.as_ref().unwrap();
        assert_eq!(link.parent_attempt_number, 1);
        assert_eq!(link.retry_of_attempt_number, Some(2));
        let calls = upstream.calls.lock().unwrap();
        assert_eq!(calls[2]["params"], calls[3]["params"]);
    }
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
    for value in [
        "private-state",
        "private-mrtr",
        "upstream-secret",
        "caller-stable-id",
    ] {
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
