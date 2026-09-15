use super::*;

pub(super) async fn respond(upstream: &Upstream, message: &Value) -> Response {
    let mut result = json!({"taskId":"private-task-handle","status":"working",
        "createdAt":"2026-09-15T00:00:00Z","lastUpdatedAt":"2026-09-15T00:00:00Z","ttlMs":null});
    if message["method"] == "tools/call" {
        result["resultType"] = "task".into();
    } else if message["method"] == "tasks/update" {
        assert_eq!(
            message["params"]["inputResponses"],
            json!({"private-input":{"action":"accept","content":{"answer":"private-answer"}}})
        );
        let sends: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mcp_operation_task_updates u JOIN mcp_operation_task_inputs i ON i.update_id = u.id WHERE u.completed_at IS NULL")
            .fetch_one(&upstream.db).await.unwrap();
        assert_eq!(
            sends, 1,
            "input consumption and update send must be durable before HTTP"
        );
        upstream.task_inputs_answered.store(true, Ordering::Release);
        result = json!({"resultType":"complete","extension":"input-ack"});
    } else if message["method"] == "tasks/cancel" {
        assert_eq!(message["params"]["taskId"], "private-task-handle");
        let sends: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM mcp_operation_task_cancellations WHERE completed_at IS NULL",
        )
        .fetch_one(&upstream.db)
        .await
        .unwrap();
        assert_eq!(sends, 1, "cancellation must be durable before HTTP");
        result = json!({"resultType":"complete","extension":"cancel-ack"});
    } else {
        assert_eq!(message["params"]["taskId"], "private-task-handle");
        let queries: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM mcp_operation_task_lookups WHERE completed_at IS NULL",
        )
        .fetch_one(&upstream.db)
        .await
        .unwrap();
        assert_eq!(queries, 1, "task lookup must be durable before HTTP");
        result["resultType"] = "complete".into();
        if upstream.task_inputs_answered.load(Ordering::Acquire) {
            result["status"] = "completed".into();
            result["result"] = json!({"content":[{"type":"text","text":"private-task-result"}],"extension":{"preserved":true}});
        } else {
            result["status"] = "input_required".into();
            result["inputRequests"] = json!({"private-input":{"method":"elicitation/create","params":{"message":"private-question"}}});
        }
    }
    Json(json!({"jsonrpc":"2.0","id":message["id"],"result":result})).into_response()
}

#[tokio::test]
async fn real_mcp_shim_links_task_inputs_and_control_acks_before_the_eventual_result() {
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
    upstream.tasks.store(true, Ordering::Release);
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
        "io.modelcontextprotocol/clientInfo":{"name":"task-provider","version":"1"},
        "io.modelcontextprotocol/clientCapabilities":{"elicitation":{"form":{}},"extensions":{"io.modelcontextprotocol/tasks":{}}}});
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
    write_message(&mut input, &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"write","arguments":{"body":"private-write"},"_meta":metadata}})).await.unwrap();
    assert_eq!(
        next(&mut output).await["result"]["taskId"],
        "private-task-handle"
    );
    let operation: String = sqlx::query_scalar("SELECT id FROM mcp_operations")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(
        journal
            .operation(&reservation.team_id, &reservation.actor_id, &operation)
            .await
            .unwrap()
            .unwrap()
            .status,
        agenthub_agent_domain::mcp_operations::McpOperationStatus::OutcomeUnknown
    );
    write_message(&mut input, &json!({"jsonrpc":"2.0","id":3,"method":"tasks/get","params":{"taskId":"foreign-task","_meta":metadata}})).await.unwrap();
    assert!(next(&mut output).await.get("error").is_some());
    assert_eq!(upstream.calls.lock().unwrap().len(), 2);
    write_message(&mut input, &json!({"jsonrpc":"2.0","id":20,"method":"tasks/get","params":{"taskId":"private-task-handle","_meta":metadata}})).await.unwrap();
    assert_eq!(
        next(&mut output).await["result"]["inputRequests"]["private-input"]["params"]["message"],
        "private-question"
    );
    write_message(&mut input, &json!({"jsonrpc":"2.0","id":21,"method":"tasks/update","params":{"taskId":"private-task-handle","inputResponses":{"private-input":{"action":"accept","content":{"answer":"private-answer"}}},"_meta":metadata}})).await.unwrap();
    assert_eq!(next(&mut output).await["result"]["extension"], "input-ack");
    write_message(&mut input, &json!({"jsonrpc":"2.0","id":22,"method":"tasks/update","params":{"taskId":"private-task-handle","inputResponses":{"private-input":{"action":"decline"}},"_meta":metadata}})).await.unwrap();
    assert!(next(&mut output).await.get("error").is_some());
    write_message(&mut input, &json!({"jsonrpc":"2.0","id":30,"method":"tasks/cancel","params":{"taskId":"private-task-handle","_meta":metadata}})).await.unwrap();
    assert_eq!(next(&mut output).await["result"]["extension"], "cancel-ack");
    assert_eq!(
        journal
            .operation(&reservation.team_id, &reservation.actor_id, &operation)
            .await
            .unwrap()
            .unwrap()
            .status,
        agenthub_agent_domain::mcp_operations::McpOperationStatus::OutcomeUnknown
    );
    write_message(&mut input, &json!({"jsonrpc":"2.0","id":31,"method":"tasks/cancel","params":{"taskId":"private-task-handle","_meta":metadata}})).await.unwrap();
    assert!(next(&mut output).await.get("error").is_some());
    write_message(&mut input, &json!({"jsonrpc":"2.0","id":4,"method":"tasks/get","params":{"taskId":"private-task-handle","_meta":metadata}})).await.unwrap();
    let response = next(&mut output).await;
    assert_eq!(
        response["result"]["result"]["content"][0]["text"],
        "private-task-result"
    );
    assert_eq!(response["result"]["result"]["extension"]["preserved"], true);
    let attempts = journal
        .attempts(
            &reservation.team_id,
            &reservation.actor_id,
            &operation,
            0,
            100,
        )
        .await
        .unwrap();
    assert_eq!(attempts.len(), 1);
    assert_eq!(
        attempts[0].status,
        agenthub_agent_domain::mcp_operations::McpOperationStatus::Succeeded
    );
    let lookups = journal
        .task_lookups(
            &reservation.team_id,
            &reservation.actor_id,
            &operation,
            0,
            100,
        )
        .await
        .unwrap();
    assert_eq!(lookups.len(), 2);
    assert!(lookups[0].outcome.is_none());
    assert!(lookups[1].outcome.is_some());
    assert_eq!(upstream.calls.lock().unwrap().len(), 6);
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
    for secret in ["private-task", "private-write", "upstream-secret"] {
        assert!(!errors.contains(secret));
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
