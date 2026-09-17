use super::*;

pub(super) fn validating_client(fixture: &Fixture) -> JournaledMcpClient {
    let manifest: agenthub_agent_domain::app_tools::AppManifest = serde_json::from_value(json!({
        "schema_version":1,"scopes":["write"],"tools":[{
            "name":"write","input_schema":{"type":"object"},
            "output_schema":{"type":"object","properties":{"saved":{"type":"boolean"}},"required":["saved"]},
            "required_scopes":["write"],"replay":{"kind":"non_idempotent"}
        }]
    })).unwrap();
    let manifest = manifest.compile().unwrap();
    JournaledMcpClient::new(
        fixture.journal.clone(),
        ByteBudget::new(8 * crate::MAX_MESSAGE_BYTES),
    )
    .validating(Some(Arc::new(move |tool, result| {
        assert_eq!(
            tool, "write",
            "the journal must resolve the original tool name"
        );
        if result["isError"] == true {
            return Ok(());
        }
        manifest
            .validate_output(tool, &result["structuredContent"])
            .map_err(|_| McpTransportError::InvalidResponse)
    })))
}

#[tokio::test]
async fn completed_results_are_validated_before_journaling_and_unsafe_writes_never_replay() {
    for (result, status) in [
        (
            json!({"content":[],"structuredContent":{"saved":true},"extension":{"native":true}}),
            McpOperationStatus::Succeeded,
        ),
        (
            json!({"content":[],"structuredContent":{"saved":"private-invalid-output"}}),
            McpOperationStatus::OutcomeUnknown,
        ),
        (
            json!({"content":[],"isError":true}),
            McpOperationStatus::Failed,
        ),
    ] {
        let fixture = Fixture::new().await;
        let executor = fixture.running().await;
        let upstream = Upstream::new(fixture.pool.clone()).await;
        *upstream.state.response.lock().unwrap() = result.clone();
        let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
        let client = validating_client(&fixture);
        let (events, mut received) = mpsc::channel(8);
        let response = client.run(prepare(&binding, &executor, 1), events).await;
        if status == McpOperationStatus::OutcomeUnknown {
            assert_eq!(
                response.err(),
                Some(McpCallError::Transport(McpTransportError::InvalidResponse))
            );
        } else {
            assert_eq!(response.unwrap().response["result"], result);
        }
        assert!(received.recv().await.is_none());
        assert_eq!(fixture.operations().await[0].status, status);
        if status == McpOperationStatus::OutcomeUnknown {
            let (events, _received) = mpsc::channel(8);
            assert!(
                client
                    .run(prepare(&binding, &executor, 2), events)
                    .await
                    .is_err()
            );
        }
        assert_eq!(upstream.count(), 1);
        drop(upstream);
        fixture.close().await;
    }
}
