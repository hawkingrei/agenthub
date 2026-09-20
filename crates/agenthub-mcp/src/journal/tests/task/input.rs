use super::*;

fn required() -> Value {
    let mut result = task_state(ProtocolVersion::July2026, "input_required");
    result["resultType"] = "complete".into();
    result["inputRequests"] = json!({
        "private-input-a":{"method":"elicitation/create","params":{"message":"private-question-a","extension":"preserved"}},
        "private-input-b":{"method":"elicitation/create","params":{"message":"private-question-b"}}
    });
    result
}

fn update(
    binding: &McpBinding,
    e: &LoopReservation,
    id: i64,
    responses: Value,
) -> crate::policy::PreparedTaskUpdate {
    let mut message = json!({"jsonrpc":"2.0","id":id,"method":"tasks/update","params":{"taskId":"private-task-id","inputResponses":responses}});
    metadata(&mut message, ProtocolVersion::July2026);
    binding
        .prepare_task_update(
            &task_catalog(ProtocolVersion::July2026),
            &McpCallContext {
                executor: e,
                proxy_session_id: "task-proxy",
                http: &HttpContext {
                    version: ProtocolVersion::July2026,
                    session_id: None,
                },
            },
            message,
        )
        .unwrap()
}

async fn send_update(
    f: &Fixture,
    call: crate::policy::PreparedTaskUpdate,
) -> Result<McpCallResult, McpCallError> {
    let (events, receiver) = mpsc::channel(8);
    drop(receiver);
    JournaledMcpClient::new(
        f.journal.clone(),
        ByteBudget::new(16 * crate::MAX_MESSAGE_BYTES),
    )
    .run_task_update(call, events)
    .await
}

async fn poll(
    f: &Fixture,
    binding: &McpBinding,
    e: &LoopReservation,
    id: i64,
) -> Result<McpCallResult, McpCallError> {
    lookup(
        f,
        query(
            binding,
            e,
            ProtocolVersion::July2026,
            id,
            "tasks/get",
            "private-task-id",
        ),
    )
    .await
}

#[tokio::test]
async fn task_inputs_survive_partial_updates_stale_polls_and_new_activations() {
    let f = Fixture::new().await;
    let e = f.running().await;
    let upstream = Upstream::new(f.pool.clone()).await;
    let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
    let created = create_task(&f, &upstream, &e, &binding, ProtocolVersion::July2026).await;
    let answer = json!({"private-input-a":{"action":"accept","content":{"answer":"private-answer"},"extension":"preserved"}});
    assert!(
        send_update(&f, update(&binding, &e, 2, answer.clone()))
            .await
            .is_err()
    );
    assert_eq!(upstream.count(), 1);
    *upstream.state.response.lock().unwrap() = required();
    assert_eq!(
        poll(&f, &binding, &e, 3).await.unwrap().response["result"],
        required()
    );
    assert!(
        send_update(
            &f,
            update(
                &binding,
                &e,
                4,
                json!({"private-input-a":{"action":"decline"},"foreign":{"action":"accept"}})
            )
        )
        .await
        .is_err()
    );
    assert_eq!(upstream.count(), 2);
    *upstream.state.response.lock().unwrap() = json!({"resultType":"complete","extension":"ack"});
    assert_eq!(
        send_update(&f, update(&binding, &e, 5, answer.clone()))
            .await
            .unwrap()
            .response["result"]["extension"],
        "ack"
    );
    assert_eq!(
        upstream.state.requests.lock().unwrap().last().unwrap()["params"]["inputResponses"],
        answer
    );
    assert_eq!(
        f.operations().await[0].status,
        McpOperationStatus::OutcomeUnknown
    );
    *upstream.state.response.lock().unwrap() = required();
    poll(&f, &binding, &e, 6).await.unwrap();
    assert!(
        send_update(&f, update(&binding, &e, 7, answer))
            .await
            .is_err()
    );
    f.stop(&e).await;
    let next = f.running().await;
    *upstream.state.response.lock().unwrap() = json!({"resultType":"complete"});
    send_update(
        &f,
        update(
            &binding,
            &next,
            8,
            json!({"private-input-b":{"action":"decline"}}),
        ),
    )
    .await
    .unwrap();
    let mut complete = task_state(ProtocolVersion::July2026, "completed");
    complete["resultType"] = "complete".into();
    complete["result"] = json!({"content":[],"isError":false});
    *upstream.state.response.lock().unwrap() = complete;
    poll(&f, &binding, &next, 9).await.unwrap();
    assert_eq!(upstream.count(), 6);
    assert_eq!(f.operations().await[0].attempt_count, 1);
    assert_eq!(
        f.operations().await[0].status,
        McpOperationStatus::Succeeded
    );
    let inputs = f
        .journal
        .task_inputs("team", "worker", &created.operation_id, 0, 100)
        .await
        .unwrap();
    let updates = f
        .journal
        .task_updates("team", "worker", &created.operation_id, 0, 100)
        .await
        .unwrap();
    assert_eq!(inputs.len(), 2);
    assert_eq!(updates.len(), 2);
    assert!(inputs.iter().all(|input| input.update_id.is_some()));
    assert!(
        !serde_json::to_string(&(inputs, updates))
            .unwrap()
            .contains("private")
    );
    f.pool.close().await;
}

#[tokio::test]
async fn task_input_ack_loss_and_rpc_failure_do_not_release_input_keys() {
    for mode in [1, 3] {
        let f = Fixture::new().await;
        let e = f.running().await;
        let upstream = Upstream::new(f.pool.clone()).await;
        let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
        create_task(&f, &upstream, &e, &binding, ProtocolVersion::July2026).await;
        *upstream.state.response.lock().unwrap() = required();
        poll(&f, &binding, &e, 2).await.unwrap();
        upstream.state.mode.store(mode, Ordering::SeqCst);
        *upstream.state.response.lock().unwrap() =
            json!({"code":-32603,"message":"private-input-error"});
        let answer = json!({"private-input-a":{"action":"accept","content":{"x":1}}});
        let result = send_update(&f, update(&binding, &e, 3, answer.clone())).await;
        if mode == 1 {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap().completion,
                McpCompletion::Failed { .. }
            ));
        }
        upstream.state.mode.store(0, Ordering::SeqCst);
        *upstream.state.response.lock().unwrap() = required();
        poll(&f, &binding, &e, 4).await.unwrap();
        f.stop(&e).await;
        let next = f.running().await;
        assert!(
            send_update(&f, update(&binding, &next, 5, answer))
                .await
                .is_err()
        );
        assert!(
            send_update(
                &f,
                update(
                    &binding,
                    &next,
                    6,
                    json!({"private-input-a":{"action":"decline"}})
                )
            )
            .await
            .is_err()
        );
        assert_eq!(upstream.count(), 4);
        assert_eq!(
            f.operations().await[0].status,
            McpOperationStatus::OutcomeUnknown
        );
        f.pool.close().await;
    }
}

#[tokio::test]
async fn task_input_key_reuse_with_changed_requests_is_not_forwarded_or_answerable() {
    let f = Fixture::new().await;
    let e = f.running().await;
    let upstream = Upstream::new(f.pool.clone()).await;
    let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
    let created = create_task(&f, &upstream, &e, &binding, ProtocolVersion::July2026).await;
    *upstream.state.response.lock().unwrap() = required();
    poll(&f, &binding, &e, 2).await.unwrap();
    let mut changed = required();
    changed["inputRequests"]["private-input-a"]["params"]["message"] =
        "different-private-question".into();
    *upstream.state.response.lock().unwrap() = changed;
    assert_eq!(
        poll(&f, &binding, &e, 3).await.err(),
        Some(McpCallError::ContinuationRequired)
    );
    assert!(
        f.journal
            .task_inputs("team", "worker", &created.operation_id, 0, 100)
            .await
            .unwrap()
            .iter()
            .any(|input| input.conflicted)
    );
    assert!(
        send_update(
            &f,
            update(
                &binding,
                &e,
                4,
                json!({"private-input-b":{"action":"accept"}})
            )
        )
        .await
        .is_err()
    );
    assert_eq!(upstream.count(), 3);
    f.pool.close().await;
}
