use super::*;
use agenthub_agent_domain::mcp_operations::McpOperationStatus;

fn task(status: &str) -> Value {
    json!({"taskId":"private-legacy-task","status":status,"ttl":null,
        "createdAt":"2026-09-16T00:00:00Z","lastUpdatedAt":"2026-09-16T00:00:00Z"})
}

fn notice(status: &str) -> Value {
    json!({"jsonrpc":"2.0","method":"notifications/tasks/status","params":task(status)})
}

pub(super) async fn respond(upstream: Arc<Upstream>, message: Value) -> Response {
    let result = match message["method"].as_str().unwrap() {
        "tools/call" => {
            assert_eq!(message["params"]["arguments"]["space_id"], "space-a");
            let sent: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM mcp_operation_attempts WHERE status='sent'",
            )
            .fetch_one(&upstream.db)
            .await
            .unwrap();
            assert_eq!(sent, 1);
            // Both streams race the creation receipt. The POST callback must reach the client
            // before either status can be correlated with an accepted task.
            upstream.task_events.send(notice("completed")).unwrap();
            upstream
                .task_events
                .send(json!({"jsonrpc":"2.0","method":"notifications/message",
                "params":{"level":"info","data":"get-notice-read"}}))
                .unwrap();
            let initial = format!(
                "data: {}\n\ndata: {}\n\n",
                notice("working"),
                json!({"jsonrpc":"2.0","id":"roots-1","method":"roots/list"})
            );
            let response =
                json!({"jsonrpc":"2.0","id":message["id"],"result":{"task":task("working")}});
            let first = futures::stream::once(async move { Ok::<_, std::io::Error>(initial) });
            let receipt = futures::stream::once(async move {
                upstream.callback.notified().await;
                Ok::<_, std::io::Error>(format!("data: {response}\n\n"))
            });
            return axum::http::Response::builder()
                .header("content-type", "text/event-stream")
                .body(axum::body::Body::from_stream(first.chain(receipt)))
                .unwrap();
        }
        "tasks/result" => {
            assert_eq!(message["params"]["taskId"], "private-legacy-task");
            let queries: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM mcp_operation_task_lookups WHERE completed_at IS NULL",
            )
            .fetch_one(&upstream.db)
            .await
            .unwrap();
            assert_eq!(queries, 1);
            json!({"content":[{"type":"text","text":"private-legacy-result"}],
                "_meta":{"io.modelcontextprotocol/related-task":{"taskId":"private-legacy-task"}}})
        }
        "ping" => json!({}),
        _ => unreachable!(),
    };
    let response = json!({"jsonrpc":"2.0","id":message["id"],"result":result});
    if message["method"] == "ping" {
        return (
            [("content-type", "text/event-stream")],
            format!("data: {}\n\ndata: {response}\n\n", notice("working")),
        )
            .into_response();
    }
    Json(response).into_response()
}

#[tokio::test]
async fn real_mcp_shim_joins_legacy_get_and_post_notices_to_the_creation_receipt() {
    let h = setup().await;
    h.upstream.legacy_tasks.store(true, Ordering::Release);
    let token = signed_token(
        &h.authz,
        &h.reservation,
        &h.run.id,
        vec![InternalAction::McpProxy.as_str().into()],
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = format!("http://{}", listener.local_addr().unwrap());
    let service = h.service.clone();
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
        actor_id: h.reservation.actor_id.clone(),
        run_id: h.run.id.clone(),
        activation_id: h.reservation.activation_id.clone().unwrap(),
        generation: h.reservation.generation,
        target,
        access_token: token,
        expires_at: chrono::Utc::now().timestamp() + 600,
        ca_cert_path: None,
    })
    .unwrap();
    let mut child = tokio::process::Command::new(
        crate::agenthub_binary::resolve_agenthub_binary_path()
            .expect("build the real binary first"),
    )
    .args(["mcp-proxy", "--server-id", "fixture"])
    .env(LOOP_CREDENTIAL_FILE_ENV, &file.path)
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .kill_on_drop(true)
    .spawn()
    .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    write_message(&mut input, &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{"roots":{}},"clientInfo":{"name":"legacy-task-fixture","version":"1"}}})).await.unwrap();
    let callback = next(&mut output).await;
    assert_eq!(callback["method"], "roots/list");
    write_message(
        &mut input,
        &json!({"jsonrpc":"2.0","id":callback["id"],"result":{"roots":[]}}),
    )
    .await
    .unwrap();
    assert_eq!(next(&mut output).await["id"], 1);
    write_message(
        &mut input,
        &json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    )
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(3), h.upstream.listened.notified())
        .await
        .unwrap();
    write_message(
        &mut input,
        &json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
    )
    .await
    .unwrap();
    assert_eq!(
        next(&mut output).await["result"]["tools"][0]["execution"]["taskSupport"],
        "optional"
    );
    write_message(
        &mut input,
        &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{
        "name":"write","arguments":{"body":"legacy-write"},"task":{}}}),
    )
    .await
    .unwrap();
    let mut callback = None;
    let mut get_observed = false;
    for _ in 0..2 {
        let message = next(&mut output).await;
        if message["method"] == "roots/list" {
            assert!(callback.replace(message).is_none());
        } else {
            assert_eq!(message["params"]["data"], "get-notice-read");
            get_observed = true;
        }
    }
    let callback = callback.expect("waiting on the task receipt cannot block callbacks");
    assert!(
        get_observed,
        "the GET must read the early notice before the POST receipt is released"
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mcp_operation_task_notifications")
        .fetch_one(&h.state.db)
        .await
        .unwrap();
    assert_eq!(count, 0);
    write_message(
        &mut input,
        &json!({"jsonrpc":"2.0","id":callback["id"],"result":{"roots":[]}}),
    )
    .await
    .unwrap();
    let mut statuses = Vec::new();
    let mut receipts = 0;
    for _ in 0..3 {
        let message = next(&mut output).await;
        if message["method"] == "notifications/tasks/status" {
            statuses.push(message["params"]["status"].as_str().unwrap().to_owned());
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM mcp_operation_task_notifications")
                    .fetch_one(&h.state.db)
                    .await
                    .unwrap();
            assert!(
                count >= statuses.len() as i64,
                "fact persistence must precede stdio delivery"
            );
        } else {
            assert_eq!(message["id"], 3);
            assert_eq!(message["result"]["task"]["taskId"], "private-legacy-task");
            receipts += 1;
        }
    }
    statuses.sort();
    assert_eq!(statuses, ["completed", "working"]);
    assert_eq!(receipts, 1);
    let operation: String = sqlx::query_scalar("SELECT id FROM mcp_operations")
        .fetch_one(&h.state.db)
        .await
        .unwrap();
    let op = h
        .journal
        .operation(&h.reservation.team_id, &h.reservation.actor_id, &operation)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        op.status,
        McpOperationStatus::OutcomeUnknown,
        "completed status is not a legacy task result"
    );
    write_message(&mut input, &json!({"jsonrpc":"2.0","id":4,"method":"tasks/result","params":{"taskId":"private-legacy-task"}})).await.unwrap();
    let result = next(&mut output).await;
    assert_eq!(
        result["result"]["content"][0]["text"],
        "private-legacy-result"
    );
    write_message(&mut input, &json!({"jsonrpc":"2.0","id":5,"method":"ping"}))
        .await
        .unwrap();
    assert_eq!(next(&mut output).await["params"]["status"], "working");
    assert_eq!(next(&mut output).await["id"], 5);
    h.upstream.task_events.send(notice("cancelled")).unwrap();
    assert_eq!(next(&mut output).await["params"]["status"], "cancelled");
    let attempts = h
        .journal
        .attempts(
            &h.reservation.team_id,
            &h.reservation.actor_id,
            &operation,
            0,
            100,
        )
        .await
        .unwrap();
    assert_eq!(attempts.len(), 1);
    assert_eq!(
        attempts[0].status,
        McpOperationStatus::Succeeded,
        "a later cancellation cannot replace the first result"
    );
    let notices = h
        .journal
        .task_notifications(
            &h.reservation.team_id,
            &h.reservation.actor_id,
            &operation,
            0,
            100,
        )
        .await
        .unwrap();
    assert_eq!(
        notices.len(),
        3,
        "the repeated control-stream notice is deduplicated"
    );
    assert_eq!(
        h.upstream
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|message| message["method"] == "tools/call")
            .count(),
        1
    );
    let stored = serde_json::to_string(&(attempts, notices)).unwrap();
    for secret in [
        "private-legacy-task",
        "private-legacy-result",
        "private-upstream-session",
        "upstream-secret",
    ] {
        assert!(!stored.contains(secret));
    }
    drop(input);
    assert!(
        tokio::time::timeout(Duration::from_secs(5), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
    assert!(h.upstream.deleted.load(Ordering::Acquire));
    h.state
        .agents
        .daemon_tasks()
        .shutdown_runtime(Duration::from_secs(3))
        .await
        .unwrap();
    grpc.abort();
    h.http.abort();
}
