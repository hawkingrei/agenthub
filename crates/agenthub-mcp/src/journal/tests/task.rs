use super::*;

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
