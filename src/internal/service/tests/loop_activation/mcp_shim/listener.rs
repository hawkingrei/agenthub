use super::*;
use crate::internal::proto::agenthub::internal::v1::{
    CloseMcpProxyRequest, ExchangeMcpProxyRequest, ListenMcpProxyRequest,
};

pub(super) async fn get(State(upstream): State<Arc<Upstream>>, headers: HeaderMap) -> Response {
    assert_eq!(headers["authorization"], "Bearer upstream-secret");
    assert_eq!(headers["mcp-session-id"], "private-upstream-session");
    let cursor = headers
        .get("last-event-id")
        .map(|cursor| cursor.to_str().unwrap().to_owned());
    upstream.gets.lock().unwrap().push(cursor.clone());
    if cursor.as_deref() == Some("private-write-cursor") {
        assert!(upstream.resume_writes.load(Ordering::Acquire));
        if upstream.expire_stream.load(Ordering::Acquire) {
            return (StatusCode::NOT_FOUND, Json(json!({"jsonrpc":"2.0","error":{"code":-32001,"message":"Session expired","data":{"restart":true}}}))).into_response();
        }
        if upstream.hold_writes.load(Ordering::Acquire) {
            upstream.write_release.notified().await;
        }
        let id = upstream
            .calls
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|message| message["method"] == "tools/call")
            .unwrap()["id"]
            .clone();
        let response = json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":"private-tool-result"}]}});
        return (
            [("content-type", "text/event-stream")],
            format!("data: {response}\n\n"),
        )
            .into_response();
    }
    if !upstream.listen_enabled.load(Ordering::Acquire) {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    if cursor.is_none() {
        upstream.listened.notify_one();
        let callback = json!({"jsonrpc":"2.0","id":"roots-listener","method":"roots/list"});
        return (
            [("content-type", "text/event-stream")],
            format!("id: private-listen-cursor\nretry: 1100\ndata: {callback}\n\n"),
        )
            .into_response();
    }
    assert_eq!(cursor.as_deref(), Some("private-listen-cursor"));
    let message = json!({"jsonrpc":"2.0","method":"notifications/message","params":{"level":"info","data":"listener-resumed"}});
    let first =
        futures::stream::once(
            async move { Ok::<_, std::io::Error>(format!("data: {message}\n\n")) },
        );
    axum::http::Response::builder()
        .header("content-type", "text/event-stream")
        .body(axum::body::Body::from_stream(
            first.chain(futures::stream::pending()),
        ))
        .unwrap()
}

pub(super) async fn delete(
    State(upstream): State<Arc<Upstream>>,
    headers: HeaderMap,
) -> StatusCode {
    assert_eq!(headers["authorization"], "Bearer upstream-secret");
    assert_eq!(headers["mcp-session-id"], "private-upstream-session");
    let sent: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM mcp_operation_attempts WHERE status='sent'")
            .fetch_one(&upstream.db)
            .await
            .unwrap();
    assert_eq!(sent, 0, "DELETE must not discard pending tool results");
    assert!(
        !upstream.deleted.swap(true, Ordering::AcqRel),
        "DELETE must be sent once"
    );
    StatusCode::NO_CONTENT
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

async fn ready(h: &Harness) -> (String, String) {
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
    let mut initialize = exchange(h, &token, &session, json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{"roots":{}},"clientInfo":{"name":"fixture","version":"1"}}})).await;
    let callback: Value =
        serde_json::from_str(&initialize.next().await.unwrap().unwrap().message_json).unwrap();
    let mut reply = exchange(
        h,
        &token,
        &session,
        json!({"jsonrpc":"2.0","id":callback["id"],"result":{"roots":[]}}),
    )
    .await;
    assert!(reply.next().await.unwrap().unwrap().finished);
    assert!(initialize.next().await.unwrap().unwrap().finished);
    let mut initialized = exchange(
        h,
        &token,
        &session,
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    )
    .await;
    assert!(initialized.next().await.unwrap().unwrap().finished);
    let mut tools = exchange(
        h,
        &token,
        &session,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
    )
    .await;
    assert!(tools.next().await.unwrap().unwrap().finished);
    (token, session)
}

#[tokio::test]
async fn mcp_expired_upstream_session_preserves_error_and_requires_new_initialization() {
    let h = setup().await;
    h.upstream.resume_writes.store(true, Ordering::Release);
    h.upstream.expire_stream.store(true, Ordering::Release);
    let (token, session) = ready(&h).await;
    let mut stream = exchange(&h, &token, &session, json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"write","arguments":{"body":"expired-write"}}})).await;
    let error = stream.next().await.unwrap().unwrap();
    assert!(!error.finished && !error.can_listen);
    assert_eq!(
        serde_json::from_str::<Value>(&error.message_json).unwrap(),
        json!({"jsonrpc":"2.0","error":{"code":-32001,"message":"Session expired","data":{"restart":true}}})
    );
    assert!(stream.next().await.is_none());
    let replacement = h
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
    assert_ne!(session, replacement);
    assert!(
        !h.state
            .agents
            .mcp_proxy()
            .unwrap()
            .session(&h.reservation, &replacement)
            .await
            .unwrap()
            .can_listen()
            .await
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
    let statuses: Vec<String> = sqlx::query_scalar("SELECT status FROM mcp_operation_attempts")
        .fetch_all(&h.state.db)
        .await
        .unwrap();
    assert_eq!(statuses, vec!["failed"]);
    h.state
        .agents
        .daemon_tasks()
        .shutdown_runtime(Duration::from_secs(3))
        .await
        .unwrap();
    h.http.abort();
}

#[tokio::test]
async fn mcp_listener_releases_executor_guard_and_delete_waits_for_resumed_write_settlement() {
    let h = setup().await;
    h.upstream.listen_enabled.store(true, Ordering::Release);
    h.upstream.resume_writes.store(true, Ordering::Release);
    let (token, session) = ready(&h).await;
    let mut listener = h
        .service
        .listen_mcp_proxy(authenticated_request(
            ListenMcpProxyRequest {
                session_id: session.clone(),
            },
            &token,
        ))
        .await
        .unwrap()
        .into_inner();
    let callback: Value =
        serde_json::from_str(&listener.next().await.unwrap().unwrap().message_json).unwrap();
    assert_eq!(callback["id"], "roots-listener");
    let gate = h
        .state
        .agents
        .loop_operation_gate(&h.reservation.actor_id)
        .await;
    let exclusive = tokio::time::timeout(Duration::from_secs(2), gate.write_owned())
        .await
        .expect("idle GET must not hold the executor guard");
    drop(exclusive);
    let mut reply = exchange(
        &h,
        &token,
        &session,
        json!({"jsonrpc":"2.0","id":callback["id"],"result":{"roots":[]}}),
    )
    .await;
    assert!(reply.next().await.unwrap().unwrap().finished);
    assert_eq!(
        serde_json::from_str::<Value>(&listener.next().await.unwrap().unwrap().message_json)
            .unwrap()["method"],
        "notifications/message"
    );
    h.upstream.hold_writes.store(true, Ordering::Release);
    let response = exchange(&h, &token, &session, json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"write","arguments":{"body":"resume-write"}}})).await;
    h.upstream.write_received.notified().await;
    drop(response);
    let close = h.service.close_mcp_proxy(authenticated_request(
        CloseMcpProxyRequest {
            session_id: session.clone(),
        },
        &token,
    ));
    tokio::pin!(close);
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut close)
            .await
            .is_err()
    );
    assert!(!h.upstream.deleted.load(Ordering::Acquire));
    h.upstream.write_release.notify_one();
    tokio::time::timeout(Duration::from_secs(3), close)
        .await
        .unwrap()
        .unwrap();
    assert!(h.upstream.deleted.load(Ordering::Acquire));
    let statuses: Vec<String> = sqlx::query_scalar("SELECT status FROM mcp_operation_attempts")
        .fetch_all(&h.state.db)
        .await
        .unwrap();
    assert_eq!(statuses, vec!["succeeded"]);
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
    assert!(
        h.upstream
            .gets
            .lock()
            .unwrap()
            .contains(&Some("private-write-cursor".into()))
    );
    h.state
        .agents
        .daemon_tasks()
        .shutdown_runtime(Duration::from_secs(3))
        .await
        .unwrap();
    h.http.abort();
}

#[tokio::test]
async fn real_mcp_shim_listens_resumes_callbacks_and_deletes_its_upstream_session() {
    let h = setup().await;
    h.upstream.listen_enabled.store(true, Ordering::Release);
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
    write_message(&mut input,&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{"roots":{}},"clientInfo":{"name":"fixture","version":"1"}}})).await.unwrap();
    let callback = next(&mut output).await;
    write_message(
        &mut input,
        &json!({"jsonrpc":"2.0","id":callback["id"],"result":{"roots":[]}}),
    )
    .await
    .unwrap();
    assert_eq!(next(&mut output).await["id"], 1);
    let started = tokio::time::Instant::now();
    write_message(
        &mut input,
        &json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    )
    .await
    .unwrap();
    let callback = next(&mut output).await;
    assert_eq!(callback["id"], "roots-listener");
    write_message(
        &mut input,
        &json!({"jsonrpc":"2.0","id":callback["id"],"result":{"roots":[]}}),
    )
    .await
    .unwrap();
    tokio::time::timeout(
        Duration::from_secs(3),
        h.upstream.listener_replied.notified(),
    )
    .await
    .unwrap();
    let notification = next(&mut output).await;
    assert_eq!(notification["params"]["data"], "listener-resumed");
    assert!(started.elapsed() >= Duration::from_millis(1100));
    assert!(!notification.to_string().contains("private-listen-cursor"));
    drop(input);
    assert!(
        tokio::time::timeout(Duration::from_secs(5), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
    assert!(h.upstream.deleted.load(Ordering::Acquire));
    assert_eq!(
        *h.upstream.gets.lock().unwrap(),
        vec![None, Some("private-listen-cursor".into())]
    );
    h.state
        .agents
        .daemon_tasks()
        .shutdown_runtime(Duration::from_secs(3))
        .await
        .unwrap();
    grpc.abort();
    h.http.abort();
}
