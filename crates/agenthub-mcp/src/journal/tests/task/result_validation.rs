use super::super::result_validation::validating_client;
use super::*;

#[tokio::test]
async fn deferred_task_results_validate_original_tool_and_reconcile_without_resending() {
    for version in [ProtocolVersion::November2025, ProtocolVersion::July2026] {
        let fixture = Fixture::new().await;
        let executor = fixture.running().await;
        let upstream = Upstream::new(fixture.pool.clone()).await;
        let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
        let client = validating_client(&fixture);
        let (events, _receiver) = mpsc::channel(8);
        let created = client
            .run(
                prepare_task(&upstream, &executor, &binding, version),
                events,
            )
            .await
            .unwrap();
        let method = if version == ProtocolVersion::November2025 {
            "tasks/result"
        } else {
            "tasks/get"
        };
        for (id, saved) in [(2, json!("private-invalid")), (3, json!(true))] {
            let mut result = json!({"content":[],"structuredContent":{"saved":saved},"extension":{"preserved":true}});
            if version == ProtocolVersion::July2026 {
                let mut task = task_state(version, "completed");
                task["resultType"] = "complete".into();
                task["result"] = result;
                result = task;
            }
            *upstream.state.response.lock().unwrap() = result.clone();
            let call = query(&binding, &executor, version, id, method, "private-task-id");
            let (events, _receiver) = mpsc::channel(8);
            let response = client.run_task_lookup(call, events).await;
            if id == 2 {
                assert_eq!(
                    response.err(),
                    Some(McpCallError::Transport(McpTransportError::InvalidResponse))
                );
                assert_ne!(
                    fixture.operations().await[0].status,
                    McpOperationStatus::Succeeded
                );
                let (events, _receiver) = mpsc::channel(8);
                assert!(
                    client
                        .run(prepare(&binding, &executor, 20), events)
                        .await
                        .is_err()
                );
                assert_eq!(upstream.count(), 2);
            } else {
                let response = response.unwrap();
                assert_eq!(response.operation_id, created.operation_id);
                assert_eq!(response.response["result"], result);
                assert_eq!(
                    fixture.operations().await[0].status,
                    McpOperationStatus::Succeeded
                );
            }
        }
        assert_eq!(upstream.count(), 3);
        assert_eq!(fixture.operations().await[0].attempt_count, 1);
        drop(upstream);
        fixture.close().await;
    }
}

#[tokio::test]
async fn task_notifications_cannot_settle_invalid_results_but_later_facts_can_reconcile() {
    let fixture = Fixture::new().await;
    let executor = fixture.running().await;
    let upstream = Upstream::new(fixture.pool.clone()).await;
    let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
    let version = ProtocolVersion::July2026;
    let created = create_task(&fixture, &upstream, &executor, &binding, version).await;
    let McpCompletion::Deferred {
        task_receipt: Some(receipt),
        ..
    } = &created.completion
    else {
        panic!("expected task");
    };
    let client = validating_client(&fixture);
    let permit = client
        .authorize_task_notifications(
            &executor,
            &binding.task_authority(&task_catalog(version)),
            receipt,
        )
        .await
        .unwrap();
    let mut params = task_state(version, "completed");
    params["result"] = json!({"content":[],"structuredContent":{"saved":"private-invalid"}});
    let mut message = json!({"jsonrpc":"2.0","method":"notifications/tasks","params":params});
    assert_eq!(
        client
            .record_task_notification(&permit, &message)
            .await
            .err(),
        Some(McpCallError::Transport(McpTransportError::InvalidResponse))
    );
    assert_eq!(
        fixture.operations().await[0].completion.as_ref(),
        Some(&created.completion)
    );
    let (events, _receiver) = mpsc::channel(8);
    assert!(
        client
            .run(prepare(&binding, &executor, 20), events)
            .await
            .is_err()
    );
    assert_eq!(upstream.count(), 1);
    message["params"]["result"]["structuredContent"]["saved"] = json!(true);
    client
        .record_task_notification(&permit, &message)
        .await
        .unwrap();
    assert_eq!(
        fixture.operations().await[0].status,
        McpOperationStatus::Succeeded
    );
    assert_eq!(
        fixture
            .journal
            .task_notifications("team", "worker", &created.operation_id, 0, 100)
            .await
            .unwrap()
            .len(),
        1
    );
    drop(upstream);
    fixture.close().await;
}
