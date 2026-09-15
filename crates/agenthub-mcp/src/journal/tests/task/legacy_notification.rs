use super::*;

fn notice(status: &str) -> Value {
    json!({"jsonrpc":"2.0","method":"notifications/tasks/status",
        "params":task_state(ProtocolVersion::November2025, status)})
}

async fn observing(f: &Fixture, e: &LoopReservation, binding: &McpBinding) -> JournaledMcpClient {
    let client =
        JournaledMcpClient::new(f.journal.clone(), ByteBudget::new(crate::MAX_MESSAGE_BYTES));
    let owner = client.task_observer(e).await.unwrap();
    let observer = owner
        .bind(
            binding.observation_binding(),
            &HttpContext {
                version: ProtocolVersion::November2025,
                session_id: None,
            },
        )
        .unwrap();
    client.observing(observer)
}

fn prepare_creation(binding: &McpBinding, e: &LoopReservation) -> PreparedToolCall {
    let mut request = message(1);
    request["params"]["task"] = json!({});
    binding
        .prepare_call(
            &task_catalog(ProtocolVersion::November2025),
            &McpCallContext {
                executor: e,
                proxy_session_id: "task-proxy",
                http: &HttpContext {
                    version: ProtocolVersion::November2025,
                    session_id: None,
                },
            },
            request,
            |_, mut arguments| {
                arguments["space_id"] = "space-a".into();
                Ok(arguments)
            },
        )
        .unwrap()
}

#[tokio::test]
async fn legacy_task_notice_before_create_receipt_keeps_callbacks_flowing() {
    let f = Fixture::new().await;
    let e = f.running().await;
    let upstream = Upstream::new(f.pool.clone()).await;
    let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
    let client = observing(&f, &e, &binding).await;
    *upstream.state.response.lock().unwrap() =
        json!({"task":task_state(ProtocolVersion::November2025, "working")});
    upstream.state.mode.store(12, Ordering::SeqCst);
    let call = prepare_creation(&binding, &e);
    let (events, mut receiver) = mpsc::channel(8);
    let running = tokio::spawn(async move { client.run(call, events).await });
    let callback = tokio::time::timeout(Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        callback.into_parts().0.message.as_ref().unwrap()["method"],
        "roots/list"
    );
    assert_eq!(f.operations().await[0].status, McpOperationStatus::Sent);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mcp_operation_task_notifications")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(
        count, 0,
        "an unaccepted task handle cannot become a durable fact"
    );
    upstream.state.release.notify_one();
    let completed = running.await.unwrap().unwrap();
    assert!(!completed.event_delivery_lost);
    assert_eq!(
        receiver
            .recv()
            .await
            .unwrap()
            .into_parts()
            .0
            .message
            .as_ref()
            .unwrap()["method"],
        "notifications/tasks/status"
    );
    assert_eq!(
        f.journal
            .task_notifications("team", "worker", &completed.operation_id, 0, 100)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        f.operations().await[0].status,
        McpOperationStatus::OutcomeUnknown,
        "legacy completed status still requires tasks/result"
    );
    upstream.state.mode.store(0, Ordering::SeqCst);
    *upstream.state.response.lock().unwrap() = json!({"content":[]});
    lookup(
        &f,
        query(
            &binding,
            &e,
            ProtocolVersion::November2025,
            2,
            "tasks/result",
            "private-task-id",
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        f.operations().await[0].status,
        McpOperationStatus::Succeeded
    );
    assert_eq!(upstream.count(), 2);
    f.close().await;
}

#[tokio::test]
async fn legacy_task_drain_bounds_unmatched_notices_and_rejects_other_protocols() {
    let f = Fixture::new().await;
    let e = f.running().await;
    let upstream = Upstream::new(f.pool.clone()).await;
    let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
    let client = observing(&f, &e, &binding).await;
    let mut drain = TaskEventDrain::new(client.observer.clone(), Duration::from_millis(20));
    assert!(matches!(
        drain.accept(&notice("working")).await,
        TaskEventDisposition::Rejected
    ));
    let pending = client.observer.as_ref().unwrap().begin_task_receipt();
    let mut drain = TaskEventDrain::new(client.observer.clone(), Duration::from_secs(1));
    for _ in 0..64 {
        assert!(matches!(
            drain.accept(&notice("working")).await,
            TaskEventDisposition::Held
        ));
    }
    assert!(matches!(
        drain.accept(&notice("working")).await,
        TaskEventDisposition::Rejected
    ));
    drop(pending);
    assert!(drain.settle().await.is_empty());
    assert!(drain.lost);
    let _pending = client.observer.as_ref().unwrap().begin_task_receipt();
    let mut expired = TaskEventDrain::new(client.observer.clone(), Duration::from_millis(20));
    assert!(matches!(
        expired.accept(&notice("working")).await,
        TaskEventDisposition::Held
    ));
    assert!(
        tokio::time::timeout(Duration::from_secs(1), expired.settle())
            .await
            .unwrap()
            .is_empty()
    );
    assert!(expired.lost);
    let mut overflow = TaskEventDrain::new(client.observer.clone(), Duration::from_secs(1));
    let mut large = notice("working");
    large["params"]["extension"] = "x".repeat(crate::MAX_MESSAGE_BYTES / 2).into();
    assert!(matches!(
        overflow.accept(&large).await,
        TaskEventDisposition::Held
    ));
    assert!(matches!(
        overflow.accept(&large).await,
        TaskEventDisposition::Rejected
    ));
    let mut unsupported = TaskEventDrain::new(None, Duration::from_secs(1));
    assert!(matches!(
        unsupported.accept(&notice("working")).await,
        TaskEventDisposition::Rejected
    ));
    let mut modern = notice("working");
    modern["method"] = "notifications/tasks".into();
    assert!(matches!(
        overflow.accept(&modern).await,
        TaskEventDisposition::Rejected
    ));
    f.close().await;
}

#[tokio::test]
async fn legacy_task_notification_fact_survives_executor_exit_and_delivery_loss() {
    let f = Fixture::new().await;
    let e = f.running().await;
    let upstream = Upstream::new(f.pool.clone()).await;
    let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
    let client = observing(&f, &e, &binding).await;
    let created = create_task(&f, &upstream, &e, &binding, ProtocolVersion::November2025).await;
    f.stop(&e).await;
    let mut drain = TaskEventDrain::new(client.observer.clone(), Duration::from_secs(1));
    assert!(matches!(
        drain.accept(&notice("cancelled")).await,
        TaskEventDisposition::Forward
    ));
    let records = f
        .journal
        .task_notifications("team", "worker", &created.operation_id, 0, 100)
        .await
        .unwrap();
    assert_eq!(records.len(), 1);
    assert!(records[0].outcome.is_some());
    let (events, receiver) = mpsc::channel(1);
    drop(receiver);
    assert!(!client.deliver(
        HttpEvent {
            message: Some(notice("cancelled")),
            cursor: None,
            retry: None
        },
        &events
    ));
    assert!(matches!(
        drain.accept(&notice("completed")).await,
        TaskEventDisposition::Forward
    ));
    assert_eq!(f.operations().await[0].status, McpOperationStatus::Failed);
    f.close().await;
}

#[tokio::test]
async fn foreign_task_notice_cannot_suppress_the_actual_creation_receipt() {
    let f = Fixture::new().await;
    let e = f.running().await;
    let upstream = Upstream::new(f.pool.clone()).await;
    let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
    let client = observing(&f, &e, &binding).await;
    upstream.state.mode.store(15, Ordering::SeqCst);
    *upstream.state.response.lock().unwrap() =
        json!({"task":task_state(ProtocolVersion::November2025, "working")});
    let (events, mut receiver) = mpsc::channel(8);
    let result = client
        .run(prepare_creation(&binding, &e), events)
        .await
        .unwrap();
    assert!(result.event_delivery_lost);
    assert!(receiver.recv().await.is_none());
    let operations = f.operations().await;
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].completion.as_ref(), Some(&result.completion));
    assert!(matches!(
        result.completion,
        McpCompletion::Deferred {
            task_receipt: Some(_),
            ..
        }
    ));
    assert!(
        f.journal
            .task_notifications("team", "worker", &result.operation_id, 0, 100)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(upstream.count(), 1);
    f.close().await;
}
