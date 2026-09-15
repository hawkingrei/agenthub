use super::*;

pub(super) fn stream(body: String) -> Response {
    axum::http::Response::builder()
        .header("content-type", "text/event-stream")
        .header("mcp-session-id", "private-session")
        .body(axum::body::Body::from(body))
        .unwrap()
}

pub(super) async fn resume(
    State(state): State<Arc<UpstreamState>>,
    headers: HeaderMap,
) -> Response {
    assert_eq!(headers["authorization"], "Bearer upstream-secret");
    assert_eq!(headers["mcp-session-id"], "private-session");
    let cursor = headers["last-event-id"].to_str().unwrap().to_owned();
    state.resumed.lock().unwrap().push(cursor.clone());
    let sent: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM mcp_operation_attempts WHERE status='sent'")
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert!(sent > 0, "resumption must retain the original send permit");
    let response = state.resumptions.lock().unwrap().remove(&cursor).unwrap();
    stream(format!("data: {response}\n\n"))
}

#[tokio::test]
async fn simultaneous_writes_resume_only_their_own_stream_without_another_post() {
    let fixture = Fixture::new().await;
    let executor = fixture.running().await;
    let upstream = Upstream::new(fixture.pool.clone()).await;
    upstream.state.mode.store(9, Ordering::SeqCst);
    let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
    let call = |id| {
        let mut message = message(id);
        message["params"]["arguments"]["body"] = json!(format!("body-{id}"));
        binding
            .prepare_call(
                &catalog(),
                &McpCallContext {
                    executor: &executor,
                    proxy_session_id: "recovery",
                    http: &HttpContext {
                        version: ProtocolVersion::November2025,
                        session_id: None,
                    },
                },
                message,
                |_, args| Ok(args),
            )
            .unwrap()
    };
    let (first, second) = tokio::join!(run(&fixture, call(1)), run(&fixture, call(2)));
    assert_eq!(first.unwrap().response["id"], 1);
    assert_eq!(second.unwrap().response["id"], 2);
    assert_eq!(upstream.count(), 2);
    let mut resumed = upstream.state.resumed.lock().unwrap().clone();
    resumed.sort();
    assert_eq!(resumed, vec!["private-stream-1", "private-stream-2"]);
    let records = fixture.operations().await;
    assert!(
        records
            .iter()
            .all(|record| record.status == McpOperationStatus::Succeeded
                && record.attempt_count == 1)
    );
    drop(upstream);
    fixture.close().await;
}

#[tokio::test]
async fn priming_metadata_does_not_consume_provider_delivery_capacity() {
    let fixture = Fixture::new().await;
    let executor = fixture.running().await;
    let upstream = Upstream::new(fixture.pool.clone()).await;
    upstream.state.mode.store(9, Ordering::SeqCst);
    let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
    let (events, mut receiver) = mpsc::channel(8);
    let result =
        JournaledMcpClient::new(fixture.journal.clone(), crate::budget::ByteBudget::new(0))
            .run(prepare(&binding, &executor, 1), events)
            .await
            .unwrap();
    assert!(!result.event_delivery_lost);
    assert!(receiver.recv().await.is_none());
    assert_eq!(result.response["id"], 1);
    assert_eq!(
        fixture.operations().await[0].status,
        McpOperationStatus::Succeeded
    );
    drop(upstream);
    fixture.close().await;
}

#[tokio::test]
async fn partial_batch_resumption_settles_only_the_original_pending_members() {
    let fixture = Fixture::new().await;
    let executor = fixture.running().await;
    let upstream = Upstream::new(fixture.pool.clone()).await;
    upstream.state.mode.store(9, Ordering::SeqCst);
    let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
    let catalog =
        McpToolCatalog::from_tools(&catalog().advertised_tools(), ProtocolVersion::March2025)
            .unwrap();
    let mut second = message(2);
    second["params"]["arguments"]["body"] = json!("second");
    let call = binding
        .prepare_batch(
            Some(&catalog),
            &McpCallContext {
                executor: &executor,
                proxy_session_id: "batch",
                http: &HttpContext {
                    version: ProtocolVersion::March2025,
                    session_id: None,
                },
            },
            json!([message(1), second]),
            |_, _, mut args| {
                args["space_id"] = json!("space-a");
                Ok(args)
            },
        )
        .unwrap();
    let (events, mut receiver) = mpsc::channel(8);
    JournaledMcpClient::new(
        fixture.journal.clone(),
        crate::budget::ByteBudget::new(crate::MAX_MESSAGE_BYTES),
    )
    .run_batch(call, events)
    .await
    .unwrap();
    assert_eq!(
        receiver.recv().await.unwrap().value.message.unwrap()["id"],
        1
    );
    assert_eq!(
        receiver.recv().await.unwrap().value.message.unwrap()[0]["id"],
        2
    );
    assert_eq!(upstream.count(), 1);
    assert_eq!(
        *upstream.state.resumed.lock().unwrap(),
        vec!["private-batch-cursor"]
    );
    let records = fixture.operations().await;
    assert_eq!(records.len(), 2);
    assert!(
        records
            .iter()
            .all(|record| record.status == McpOperationStatus::Succeeded
                && record.attempt_count == 1)
    );
    drop(upstream);
    fixture.close().await;
}

#[tokio::test]
async fn retry_beyond_deadline_and_cleared_cursor_never_trigger_a_post_or_early_get() {
    for mode in [10, 11] {
        let fixture = Fixture::new().await;
        let executor = fixture.running().await;
        let upstream = Upstream::new(fixture.pool.clone()).await;
        upstream.state.mode.store(mode, Ordering::SeqCst);
        let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
        let error = run(&fixture, prepare(&binding, &executor, 1))
            .await
            .err()
            .unwrap();
        assert_eq!(
            error,
            McpCallError::Transport(if mode == 10 {
                McpTransportError::Deadline
            } else {
                McpTransportError::Disconnected
            })
        );
        assert_eq!(upstream.count(), 1);
        assert!(upstream.state.resumed.lock().unwrap().is_empty());
        assert_eq!(
            fixture.operations().await[0].status,
            McpOperationStatus::OutcomeUnknown
        );
        drop(upstream);
        fixture.close().await;
    }
}
