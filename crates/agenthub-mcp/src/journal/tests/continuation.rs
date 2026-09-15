use super::*;

fn modern_message(id: i64) -> Value {
    let mut request = message(id);
    request["params"]["_meta"] = json!({
        "io.modelcontextprotocol/protocolVersion":"2026-07-28",
        "io.modelcontextprotocol/clientInfo":{"name":"fixture","version":"1"},
        "io.modelcontextprotocol/clientCapabilities":{"elicitation":{"form":{}},"roots":{}}
    });
    request
}

fn modern_call(
    binding: &McpBinding,
    executor: &LoopReservation,
    request: Value,
) -> PreparedToolCall {
    let catalog =
        McpToolCatalog::from_tools(&catalog().advertised_tools(), ProtocolVersion::July2026)
            .unwrap();
    binding
        .prepare_call(
            &catalog,
            &McpCallContext {
                executor,
                proxy_session_id: "modern-proxy",
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
}

fn required(state: &str) -> Value {
    json!({"resultType":"input_required","requestState":state,"inputRequests":{
        "private-confirmation":{"method":"elicitation/create","params":{"mode":"form","message":"Confirm","requestedSchema":{"type":"object"}}}
    }})
}

#[tokio::test]
async fn mrtr_rounds_preserve_bound_parameters_and_land_one_linked_operation() {
    let fixture = Fixture::new().await;
    let executor = fixture.running().await;
    let upstream = Upstream::new(fixture.pool.clone()).await;
    let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
    *upstream.state.response.lock().unwrap() = required("private-state-one");
    let first = run(
        &fixture,
        modern_call(&binding, &executor, modern_message(1)),
    )
    .await
    .unwrap();
    let mut follow = modern_message(2);
    follow["params"]["requestState"] = "private-state-one".into();
    // Results are direct MCP input results, not JSON-RPC response envelopes.
    follow["params"]["inputResponses"] =
        json!({"private-confirmation":{"action":"accept","content":{"confirmed":true}}});
    for field in ["state", "arguments", "metadata"] {
        let mut invalid = follow.clone();
        match field {
            "state" => invalid["params"]["requestState"] = "another-state".into(),
            "arguments" => invalid["params"]["arguments"]["body"] = "changed-body".into(),
            _ => invalid["params"]["_meta"]["extension"] = true.into(),
        }
        assert_eq!(
            run(&fixture, modern_call(&binding, &executor, invalid))
                .await
                .err(),
            Some(McpCallError::ContinuationRequired)
        );
    }
    assert_eq!(upstream.count(), 1);
    *upstream.state.response.lock().unwrap() =
        json!({"resultType":"input_required","requestState":"private-state-two"});
    let second = run(&fixture, modern_call(&binding, &executor, follow.clone()))
        .await
        .unwrap();
    assert_eq!(second.operation_id, first.operation_id);
    assert_eq!(second.attempt_number, 2);
    let mut last = modern_message(3);
    last["params"]["requestState"] = "private-state-two".into();
    *upstream.state.response.lock().unwrap() =
        json!({"resultType":"complete","content":[{"type":"text","text":"private-final-result"}]});
    let third = run(&fixture, modern_call(&binding, &executor, last))
        .await
        .unwrap();
    assert_eq!(third.operation_id, first.operation_id);
    assert_eq!(third.attempt_number, 3);
    let requests = upstream.state.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests[1]["params"]["requestState"],
        follow["params"]["requestState"]
    );
    assert_eq!(
        requests[1]["params"]["inputResponses"],
        follow["params"]["inputResponses"]
    );
    assert!(requests[2]["params"].get("inputResponses").is_none());
    for request in &requests {
        assert_eq!(
            request["params"]["arguments"],
            requests[0]["params"]["arguments"]
        );
    }
    let records = fixture.operations().await;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].status, McpOperationStatus::Succeeded);
    let attempts = fixture
        .journal
        .attempts("team", "worker", &first.operation_id, 0, 100)
        .await
        .unwrap();
    assert_eq!(
        attempts[1]
            .continuation
            .as_ref()
            .unwrap()
            .parent_attempt_number,
        1
    );
    assert_eq!(
        attempts[2]
            .continuation
            .as_ref()
            .unwrap()
            .parent_attempt_number,
        2
    );
    let stored = serde_json::to_string(&(records, attempts)).unwrap();
    for value in [
        "private-state",
        "private-confirmation",
        "confirmed",
        "private-final-result",
        "private-arguments",
    ] {
        assert!(!stored.contains(value));
    }
    drop(upstream);
    fixture.close().await;
}

#[tokio::test]
async fn mrtr_lost_continuation_receipt_blocks_both_resend_and_initial_write() {
    let fixture = Fixture::new().await;
    let executor = fixture.running().await;
    let upstream = Upstream::new(fixture.pool.clone()).await;
    let binding = upstream.binding(TrustedReplayPolicy::StableIdentity {
        property_path: vec!["request_id".into()],
    });
    *upstream.state.response.lock().unwrap() = required("private-state");
    let first = run(
        &fixture,
        modern_call(&binding, &executor, modern_message(1)),
    )
    .await
    .unwrap();
    let mut follow = modern_message(2);
    follow["params"]["requestState"] = "private-state".into();
    follow["params"]["inputResponses"] = json!({"private-confirmation":{"action":"cancel"}});
    upstream.state.mode.store(1, Ordering::SeqCst);
    assert!(matches!(
        run(&fixture, modern_call(&binding, &executor, follow.clone())).await,
        Err(McpCallError::Transport(_))
    ));
    follow["id"] = json!(3);
    assert_eq!(
        run(&fixture, modern_call(&binding, &executor, follow))
            .await
            .err(),
        Some(McpCallError::ContinuationRequired)
    );
    assert_eq!(
        run(
            &fixture,
            modern_call(&binding, &executor, modern_message(4))
        )
        .await
        .err(),
        Some(McpCallError::ContinuationRequired)
    );
    let attempts = fixture
        .journal
        .attempts("team", "worker", &first.operation_id, 0, 100)
        .await
        .unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[1].status, McpOperationStatus::OutcomeUnknown);
    assert_eq!(upstream.count(), 2);
    drop(upstream);
    fixture.close().await;
}

#[tokio::test]
async fn mrtr_missing_state_and_partial_inputs_reach_the_original_upstream_request() {
    let fixture = Fixture::new().await;
    let executor = fixture.running().await;
    let upstream = Upstream::new(fixture.pool.clone()).await;
    let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
    let mut response = required("unused");
    response.as_object_mut().unwrap().remove("requestState");
    response["inputRequests"]["another-input"] = json!({"method":"roots/list"});
    *upstream.state.response.lock().unwrap() = response;
    let first = run(
        &fixture,
        modern_call(&binding, &executor, modern_message(1)),
    )
    .await
    .unwrap();
    let mut follow = modern_message(2);
    follow["params"]["inputResponses"] =
        json!({"private-confirmation":{"action":"cancel"},"extra":{"action":"decline"}});
    *upstream.state.response.lock().unwrap() = json!({"resultType":"complete","content":[]});
    let result = run(&fixture, modern_call(&binding, &executor, follow.clone()))
        .await
        .unwrap();
    assert_eq!(result.operation_id, first.operation_id);
    {
        let requests = upstream.state.requests.lock().unwrap();
        assert!(requests[1]["params"].get("requestState").is_none());
        assert_eq!(
            requests[1]["params"]["inputResponses"],
            follow["params"]["inputResponses"]
        );
    }
    drop(upstream);
    fixture.close().await;
}
