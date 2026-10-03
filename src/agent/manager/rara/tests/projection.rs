use agenthub_agent_event_codec::decode_message_from_storage;

use super::*;
use crate::agent::AgentSendInputError;

#[tokio::test]
async fn receipt_history_failure_preserves_the_durable_input_outcome() {
    for (scenario, follow_up, expected) in [
        ("input_ack_only", false, "accepted"),
        ("input_ack_only", true, "queued"),
        ("input_reject", false, "rejected"),
        ("input_drop", false, "outcome_unknown"),
    ] {
        let fixture = Fixture::new(scenario).await;
        let session = fixture
            .manager
            .start_agent(&fixture.agent_id)
            .await
            .unwrap();
        let runtime = fixture.runtime().await;
        let pool = fixture
            .manager
            .event_dbs
            .pool_for_agent(&fixture.agent_id)
            .await
            .unwrap();
        sqlx::query("CREATE TRIGGER reject_receipt_history BEFORE INSERT ON agent_events WHEN NEW.stream = 'acp' AND EXISTS (SELECT 1 FROM runtime_control_receipts WHERE request_id = 'input' AND status IN ('accepted', 'queued', 'rejected', 'outcome_unknown')) BEGIN SELECT RAISE(ABORT, 'fixture receipt projection failure'); END")
            .execute(&pool).await.unwrap();
        let result = runtime
            .send_input(
                "Keep this input",
                Some("input"),
                None,
                follow_up.then(|| serde_json::json!({"type":"fixture"})),
            )
            .await;
        let status = fixture.input_receipt("input").await.0;
        let stored: Vec<Vec<u8>> =
            sqlx::query_scalar("SELECT message FROM agent_events WHERE stream = 'acp'")
                .fetch_all(&pool)
                .await
                .unwrap();
        sqlx::query("DROP TRIGGER reject_receipt_history")
            .execute(&pool)
            .await
            .unwrap();
        let history = fixture
            .manager
            .list_events_for_session(&fixture.agent_id, &session, 100, None)
            .await
            .unwrap();
        let requests = fixture.input_requests();
        fixture.finish().await;

        if matches!(expected, "accepted" | "queued") {
            result.unwrap();
        } else {
            assert!(matches!(
                result.unwrap_err().downcast_ref::<AgentSendInputError>(),
                Some(AgentSendInputError::NativeInputNotAccepted { .. })
            ));
        }
        assert_eq!(status, expected);
        assert_eq!(
            requests.len(),
            1,
            "projection repair cannot dispatch another input"
        );
        let stored: Vec<Value> = stored
            .iter()
            .map(|bytes| {
                serde_json::from_str::<Value>(&decode_message_from_storage(bytes)).unwrap()
            })
            .filter(|value| value["message_id"] == "input")
            .collect();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0]["type"], "user_message");
        assert_eq!(stored[0]["meta"]["delivery"], "pending");
        let message: Value = history
            .iter()
            .filter_map(|entry| serde_json::from_str::<Value>(&entry.message).ok())
            .find(|value| value["type"] == "user_message")
            .unwrap();
        assert_eq!(message["meta"]["delivery"], expected);
    }
}
