use super::*;

fn cancellation(
    binding: &McpBinding,
    executor: &LoopReservation,
    version: ProtocolVersion,
    id: i64,
    handle: &str,
) -> crate::policy::PreparedTaskCancellation {
    let mut request =
        json!({"jsonrpc":"2.0","id":id,"method":"tasks/cancel","params":{"taskId":handle}});
    metadata(&mut request, version);
    binding
        .prepare_task_cancellation(
            &task_catalog(version),
            &McpCallContext {
                executor,
                proxy_session_id: "task-proxy",
                http: &HttpContext {
                    version,
                    session_id: None,
                },
            },
            request,
        )
        .unwrap()
}

async fn cancel(
    fixture: &Fixture,
    call: crate::policy::PreparedTaskCancellation,
) -> Result<McpCallResult, McpCallError> {
    let (events, receiver) = mpsc::channel(8);
    drop(receiver);
    JournaledMcpClient::new(
        fixture.journal.clone(),
        ByteBudget::new(16 * crate::MAX_MESSAGE_BYTES),
    )
    .run_task_cancellation(call, events)
    .await
}

#[tokio::test]
async fn task_cancellation_distinguishes_modern_ack_from_legacy_cancelled_status() {
    for version in [ProtocolVersion::November2025, ProtocolVersion::July2026] {
        let fixture = Fixture::new().await;
        let executor = fixture.running().await;
        let upstream = Upstream::new(fixture.pool.clone()).await;
        let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
        let created = create_task(&fixture, &upstream, &executor, &binding, version).await;
        assert_eq!(
            cancel(
                &fixture,
                cancellation(&binding, &executor, version, 2, "foreign-task")
            )
            .await
            .err(),
            Some(McpCallError::ContinuationRequired)
        );
        let result = if version == ProtocolVersion::July2026 {
            json!({"resultType":"complete","extension":"preserved"})
        } else {
            task_state(version, "cancelled")
        };
        *upstream.state.response.lock().unwrap() = result.clone();
        let response = cancel(
            &fixture,
            cancellation(&binding, &executor, version, 3, "private-task-id"),
        )
        .await
        .unwrap();
        assert_eq!(response.response["result"], result);
        assert!(matches!(
            response.completion,
            McpCompletion::Succeeded { .. }
        ));
        let cancellation = fixture
            .journal
            .task_cancellation("team", "worker", &created.operation_id, 1)
            .await
            .unwrap()
            .unwrap();
        if version == ProtocolVersion::July2026 {
            assert!(cancellation.outcome.is_none());
            assert!(matches!(
                fixture.operations().await[0].completion,
                Some(McpCompletion::Deferred { .. })
            ));
            let mut done = task_state(version, "completed");
            done["resultType"] = "complete".into();
            done["result"] = json!({"content":[],"isError":false});
            *upstream.state.response.lock().unwrap() = done;
            lookup(
                &fixture,
                query(
                    &binding,
                    &executor,
                    version,
                    4,
                    "tasks/get",
                    "private-task-id",
                ),
            )
            .await
            .unwrap();
            assert_eq!(
                fixture.operations().await[0].status,
                McpOperationStatus::Succeeded
            );
        } else {
            assert!(matches!(
                cancellation.outcome,
                Some(McpCompletion::Failed {
                    reason: agenthub_agent_domain::mcp_operations::McpFailureKind::TaskCancelled,
                    ..
                })
            ));
            assert_eq!(
                fixture.operations().await[0].status,
                McpOperationStatus::Failed
            );
        }
        assert_eq!(fixture.operations().await[0].attempt_count, 1);
        assert_eq!(
            upstream.count(),
            if version == ProtocolVersion::July2026 {
                3
            } else {
                2
            }
        );
        fixture.pool.close().await;
    }
}

#[tokio::test]
async fn task_cancellation_errors_and_lost_ack_never_replay_or_complete_the_tool() {
    for (version, mode, result) in [
        (
            ProtocolVersion::July2026,
            1,
            json!({"resultType":"complete"}),
        ),
        (
            ProtocolVersion::July2026,
            3,
            json!({"code":-32603,"message":"private-error"}),
        ),
        (ProtocolVersion::July2026, 0, json!({})),
        (
            ProtocolVersion::November2025,
            0,
            task_state(ProtocolVersion::November2025, "working"),
        ),
    ] {
        let fixture = Fixture::new().await;
        let executor = fixture.running().await;
        let upstream = Upstream::new(fixture.pool.clone()).await;
        let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
        let created = create_task(&fixture, &upstream, &executor, &binding, version).await;
        *upstream.state.response.lock().unwrap() = result;
        upstream.state.mode.store(mode, Ordering::SeqCst);
        let response = cancel(
            &fixture,
            cancellation(&binding, &executor, version, 2, "private-task-id"),
        )
        .await;
        if mode == 3 {
            assert!(matches!(
                response.unwrap().completion,
                McpCompletion::Failed { .. }
            ));
        } else {
            assert!(response.is_err());
        }
        assert!(matches!(
            fixture.operations().await[0].completion,
            Some(McpCompletion::Deferred { .. })
        ));
        fixture.stop(&executor).await;
        let active = fixture.running().await;
        assert!(
            cancel(
                &fixture,
                cancellation(&binding, &active, version, 3, "private-task-id")
            )
            .await
            .is_err()
        );
        assert_eq!(upstream.count(), 2);
        assert!(
            fixture
                .journal
                .task_cancellation("team", "worker", &created.operation_id, 1)
                .await
                .unwrap()
                .unwrap()
                .outcome
                .is_none()
        );
        fixture.pool.close().await;
    }
}
