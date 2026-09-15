use super::*;
mod cancellation;
mod input;
mod notification;

fn task_state(version: ProtocolVersion, status: &str) -> Value {
    let mut task = json!({"taskId":"private-task-id","status":status,
        "createdAt":"2026-09-15T00:00:00Z","lastUpdatedAt":"2026-09-15T00:00:00Z"});
    task[if version == ProtocolVersion::July2026 {
        "ttlMs"
    } else {
        "ttl"
    }] = Value::Null;
    task
}

fn task_catalog(version: ProtocolVersion) -> McpToolCatalog {
    let mut tools = catalog().advertised_tools();
    tools[0]["execution"] = json!({"taskSupport":"optional"});
    McpToolCatalog::from_tools(&tools, version).unwrap()
}

fn metadata(request: &mut Value, version: ProtocolVersion) {
    if version == ProtocolVersion::July2026 {
        request["params"]["_meta"] = json!({"io.modelcontextprotocol/protocolVersion":"2026-07-28",
            "io.modelcontextprotocol/clientInfo":{"name":"fixture","version":"1"},
            "io.modelcontextprotocol/clientCapabilities":{"elicitation":{"form":{}},"extensions":{"io.modelcontextprotocol/tasks":{}}}});
    }
}

async fn create_task(
    fixture: &Fixture,
    upstream: &Upstream,
    executor: &LoopReservation,
    binding: &McpBinding,
    version: ProtocolVersion,
) -> McpCallResult {
    let mut request = message(1);
    metadata(&mut request, version);
    let mut task = task_state(version, "working");
    if version == ProtocolVersion::July2026 {
        task["resultType"] = "task".into();
    } else {
        request["params"]["task"] = json!({});
        task = json!({"task":task});
    }
    *upstream.state.response.lock().unwrap() = task;
    let call = binding
        .prepare_call(
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
            |_, mut arguments| {
                arguments["space_id"] = "space-a".into();
                Ok(arguments)
            },
        )
        .unwrap();
    run(fixture, call).await.unwrap()
}

fn query(
    binding: &McpBinding,
    executor: &LoopReservation,
    version: ProtocolVersion,
    id: i64,
    method: &str,
    task_id: &str,
) -> crate::policy::PreparedTaskLookup {
    let mut request = json!({"jsonrpc":"2.0","id":id,"method":method,"params":{"taskId":task_id}});
    metadata(&mut request, version);
    binding
        .prepare_task_lookup(
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

async fn lookup(
    fixture: &Fixture,
    call: crate::policy::PreparedTaskLookup,
) -> Result<McpCallResult, McpCallError> {
    let (events, receiver) = mpsc::channel(8);
    drop(receiver);
    JournaledMcpClient::new(
        fixture.journal.clone(),
        ByteBudget::new(16 * crate::MAX_MESSAGE_BYTES),
    )
    .run_task_lookup(call, events)
    .await
}

#[tokio::test]
async fn task_queries_resolve_the_original_operation_across_activations_without_another_write() {
    for version in [ProtocolVersion::November2025, ProtocolVersion::July2026] {
        let fixture = Fixture::new().await;
        let executor = fixture.running().await;
        let upstream = Upstream::new(fixture.pool.clone()).await;
        let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
        let created = create_task(&fixture, &upstream, &executor, &binding, version).await;
        assert!(matches!(
            created.completion,
            McpCompletion::Deferred {
                task_receipt: Some(_),
                ..
            }
        ));
        assert_eq!(
            lookup(
                &fixture,
                query(&binding, &executor, version, 2, "tasks/get", "foreign-task")
            )
            .await
            .err(),
            Some(McpCallError::ContinuationRequired)
        );
        assert_eq!(upstream.count(), 1);
        upstream.state.mode.store(1, Ordering::SeqCst);
        assert!(
            lookup(
                &fixture,
                query(
                    &binding,
                    &executor,
                    version,
                    3,
                    "tasks/get",
                    "private-task-id"
                )
            )
            .await
            .is_err()
        );
        assert_eq!(
            fixture.operations().await[0].status,
            McpOperationStatus::OutcomeUnknown
        );
        fixture.stop(&executor).await;
        let active = fixture.running().await;
        upstream.state.mode.store(0, Ordering::SeqCst);
        let mut task = task_state(version, "completed");
        let result = json!({"content":[{"type":"text","text":"private-task-result"}],"extension":{"preserved":true}});
        if version == ProtocolVersion::July2026 {
            task["resultType"] = "complete".into();
            task["result"] = result.clone();
        }
        *upstream.state.response.lock().unwrap() = task.clone();
        let response = lookup(
            &fixture,
            query(
                &binding,
                &active,
                version,
                4,
                "tasks/get",
                "private-task-id",
            ),
        )
        .await
        .unwrap();
        assert_eq!(response.response["result"], task);
        assert_eq!(response.operation_id, created.operation_id);
        if version == ProtocolVersion::November2025 {
            assert_eq!(
                fixture.operations().await[0].status,
                McpOperationStatus::OutcomeUnknown
            );
            *upstream.state.response.lock().unwrap() = result.clone();
            let response = lookup(
                &fixture,
                query(
                    &binding,
                    &active,
                    version,
                    5,
                    "tasks/result",
                    "private-task-id",
                ),
            )
            .await
            .unwrap();
            assert_eq!(response.response["result"], result);
        }
        let operations = fixture.operations().await;
        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0].status, McpOperationStatus::Succeeded);
        let attempts = fixture
            .journal
            .attempts("team", "worker", &created.operation_id, 0, 100)
            .await
            .unwrap();
        assert_eq!(attempts.len(), 1);
        let lookups = fixture
            .journal
            .task_lookups("team", "worker", &created.operation_id, 0, 100)
            .await
            .unwrap();
        assert!(matches!(
            lookups[0].completion,
            Some(McpCompletion::OutcomeUnknown { .. })
        ));
        assert!(lookups.last().unwrap().outcome.is_some());
        let stored = serde_json::to_string(&(operations, attempts, lookups)).unwrap();
        for private in [
            "private-task-id",
            "private-task-result",
            "private-arguments",
            "upstream-secret",
        ] {
            assert!(!stored.contains(private));
        }
        assert_eq!(
            upstream
                .state
                .requests
                .lock()
                .unwrap()
                .iter()
                .filter(|request| request["method"] == "tools/call")
                .count(),
            1
        );
        drop(upstream);
        fixture.close().await;
    }
}

#[tokio::test]
async fn modern_task_query_errors_and_foreign_handles_cannot_claim_a_tool_outcome() {
    let fixture = Fixture::new().await;
    let executor = fixture.running().await;
    let upstream = Upstream::new(fixture.pool.clone()).await;
    let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
    let version = ProtocolVersion::July2026;
    let created = create_task(&fixture, &upstream, &executor, &binding, version).await;
    upstream.state.mode.store(3, Ordering::SeqCst);
    *upstream.state.response.lock().unwrap() =
        json!({"code":-32602,"message":"Task not found","data":{"private":"preserved"}});
    let result = lookup(
        &fixture,
        query(
            &binding,
            &executor,
            version,
            2,
            "tasks/get",
            "private-task-id",
        ),
    )
    .await
    .unwrap();
    assert_eq!(result.response["error"]["data"]["private"], "preserved");
    upstream.state.mode.store(0, Ordering::SeqCst);
    let mut task = task_state(version, "completed");
    task["resultType"] = "complete".into();
    task["result"] = json!({"content":[]});
    task["taskId"] = "foreign-task".into();
    *upstream.state.response.lock().unwrap() = task.clone();
    assert!(
        lookup(
            &fixture,
            query(
                &binding,
                &executor,
                version,
                3,
                "tasks/get",
                "private-task-id"
            )
        )
        .await
        .is_err()
    );
    assert_eq!(
        fixture.operations().await[0].status,
        McpOperationStatus::OutcomeUnknown
    );
    task["taskId"] = "private-task-id".into();
    task["result"] = json!({"content":[],"isError":true});
    *upstream.state.response.lock().unwrap() = task;
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
    assert!(matches!(
        fixture.operations().await[0].completion,
        Some(McpCompletion::Failed {
            reason: agenthub_agent_domain::mcp_operations::McpFailureKind::McpResult,
            ..
        })
    ));
    let rows = fixture
        .journal
        .task_lookups("team", "worker", &created.operation_id, 0, 100)
        .await
        .unwrap();
    assert!(rows[0].outcome.is_none());
    assert!(rows[1].outcome.is_none());
    assert!(rows[2].outcome.is_some());
    drop(upstream);
    fixture.close().await;
}

#[tokio::test]
async fn task_terminal_failures_and_invalid_completed_responses_never_report_success() {
    for (version, status) in [
        (ProtocolVersion::July2026, "failed"),
        (ProtocolVersion::July2026, "cancelled"),
        (ProtocolVersion::November2025, "failed"),
        (ProtocolVersion::November2025, "cancelled"),
        (ProtocolVersion::July2026, "completed"),
    ] {
        let fixture = Fixture::new().await;
        let executor = fixture.running().await;
        let upstream = Upstream::new(fixture.pool.clone()).await;
        let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
        create_task(&fixture, &upstream, &executor, &binding, version).await;
        let mut task = task_state(version, status);
        if version == ProtocolVersion::July2026 {
            task["resultType"] = "complete".into();
        }
        if status == "failed" && version == ProtocolVersion::July2026 {
            task["error"] = json!({"code":-32603,"message":"private-task-failure"});
        }
        *upstream.state.response.lock().unwrap() = task.clone();
        let response = lookup(
            &fixture,
            query(
                &binding,
                &executor,
                version,
                2,
                "tasks/get",
                "private-task-id",
            ),
        )
        .await;
        if status == "completed" {
            assert!(
                response.is_err(),
                "missing actual result cannot establish completion"
            );
            assert_eq!(
                fixture.operations().await[0].status,
                McpOperationStatus::OutcomeUnknown
            );
        } else {
            assert_eq!(response.unwrap().response["result"], task);
            assert_eq!(
                fixture.operations().await[0].status,
                McpOperationStatus::Failed
            );
        }
        drop(upstream);
        fixture.close().await;
    }
}

#[tokio::test]
async fn task_lookup_policy_rejects_missing_handles_capabilities_and_wrong_wire_era() {
    let fixture = Fixture::new().await;
    let executor = fixture.running().await;
    let upstream = Upstream::new(fixture.pool.clone()).await;
    let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
    for case in [
        "missing_id",
        "missing_capability",
        "modern_result",
        "legacy_batch_era",
        "input_response",
    ] {
        let version = if case == "legacy_batch_era" {
            ProtocolVersion::March2025
        } else {
            ProtocolVersion::July2026
        };
        let mut request = json!({"jsonrpc":"2.0","id":2,"method":"tasks/get","params":{"taskId":"private-task-id"}});
        metadata(&mut request, version);
        match case {
            "missing_id" => {
                request["params"].as_object_mut().unwrap().remove("taskId");
            }
            "missing_capability" => {
                request["params"]["_meta"]["io.modelcontextprotocol/clientCapabilities"] = json!({})
            }
            "modern_result" => request["method"] = "tasks/result".into(),
            "input_response" => request["params"]["inputResponses"] = json!({}),
            _ => (),
        }
        assert!(
            binding
                .prepare_task_lookup(
                    &task_catalog(version),
                    &McpCallContext {
                        executor: &executor,
                        proxy_session_id: "task-proxy",
                        http: &HttpContext {
                            version,
                            session_id: None
                        }
                    },
                    request
                )
                .is_err(),
            "{case}"
        );
    }
    assert_eq!(upstream.count(), 0);
    drop(upstream);
    fixture.close().await;
}

#[tokio::test]
async fn modern_task_handles_are_deferred_even_when_the_initial_status_is_terminal() {
    // The released Tasks extension uses Result & Task, without the legacy task wrapper.
    for status in [
        "working",
        "input_required",
        "completed",
        "failed",
        "cancelled",
    ] {
        let fixture = Fixture::new().await;
        let executor = fixture.running().await;
        let upstream = Upstream::new(fixture.pool.clone()).await;
        let receipt = json!({
            "resultType":"task", "taskId":"private-task-id", "status":status,
            "createdAt":"2026-09-15T00:00:00Z", "lastUpdatedAt":"2026-09-15T00:00:00Z",
            "ttlMs":60000, "pollIntervalMs":5000, "extension":{"preserved":true}
        });
        *upstream.state.response.lock().unwrap() = receipt.clone();
        let binding = upstream.binding(TrustedReplayPolicy::StableIdentity {
            property_path: vec!["request_id".into()],
        });
        let catalog =
            McpToolCatalog::from_tools(&catalog().advertised_tools(), ProtocolVersion::July2026)
                .unwrap();
        let prepare = |id| {
            let mut request = message(id);
            request["params"]["_meta"] = json!({
                "io.modelcontextprotocol/protocolVersion":"2026-07-28",
                "io.modelcontextprotocol/clientInfo":{"name":"tasks-fixture","version":"1"},
                "io.modelcontextprotocol/clientCapabilities":{"extensions":{"io.modelcontextprotocol/tasks":{}}}
            });
            binding
                .prepare_call(
                    &catalog,
                    &McpCallContext {
                        executor: &executor,
                        proxy_session_id: "tasks-proxy",
                        http: &HttpContext {
                            version: ProtocolVersion::July2026,
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
        };
        let result = run(&fixture, prepare(1)).await.unwrap();
        assert_eq!(result.response["result"], receipt);
        assert!(
            matches!(
                result.completion,
                McpCompletion::Deferred {
                    reason: McpDeferralKind::TaskAccepted,
                    ..
                }
            ),
            "{status}"
        );
        let operations = fixture.operations().await;
        assert_eq!(operations[0].status, McpOperationStatus::OutcomeUnknown);
        assert_eq!(
            run(&fixture, prepare(2)).await.err(),
            Some(McpCallError::ContinuationRequired)
        );
        assert_eq!(upstream.count(), 1);
        assert!(
            !serde_json::to_string(&operations)
                .unwrap()
                .contains("private-task-id")
        );
        drop(upstream);
        fixture.close().await;
    }
}
