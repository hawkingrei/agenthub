use super::*;
use crate::agent::{AgentOutput, AgentSendInputError};

pub(super) async fn output(
    receiver: &mut tokio::sync::broadcast::Receiver<AgentOutput>,
    kind: &str,
) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let entry = receiver.recv().await.unwrap();
            if let Ok(value) = serde_json::from_str::<Value>(&entry.message)
                && value["type"] == kind
            {
                return value;
            }
        }
    })
    .await
    .unwrap()
}

impl Fixture {
    pub(super) async fn input_receipt(&self, id: &str) -> (String, Option<String>) {
        let pool = self
            .manager
            .event_dbs
            .pool_for_agent(&self.agent_id)
            .await
            .unwrap();
        sqlx::query_as("SELECT status, ack_json FROM runtime_control_receipts WHERE request_id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap()
    }

    pub(super) fn input_requests(&self) -> Vec<Value> {
        std::fs::read_to_string(self.directory.join("requests.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}

#[tokio::test]
async fn inputs_keep_single_request_identity_and_distinct_delivery_receipts() {
    let fixture = Fixture::new("normal").await;
    let session = fixture
        .manager
        .start_agent(&fixture.agent_id)
        .await
        .unwrap();
    let mut receiver = fixture
        .manager
        .subscribe_output(&fixture.agent_id)
        .await
        .unwrap();
    fixture
        .manager
        .send_input(&fixture.agent_id, "first", Some("first"), Some(&session))
        .await
        .unwrap();
    // Receipt completion and stream delivery have independent scheduling.
    let update = output(&mut receiver, "run_status").await;
    assert_eq!(update["status"], "running");
    fixture
        .manager
        .send_input(&fixture.agent_id, "second", Some("second"), Some(&session))
        .await
        .unwrap();
    assert_eq!(fixture.input_receipt("first").await.0, "accepted");
    assert_eq!(fixture.input_receipt("second").await.0, "queued");
    let error = fixture
        .manager
        .send_input(
            &fixture.agent_id,
            "replacement",
            Some("first"),
            Some(&session),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<AgentSendInputError>(),
        Some(AgentSendInputError::NativeRequestReused { .. })
    ));
    let requests = fixture.input_requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[1]["payload"]["envelope"]["request"]["payload"]["type"],
        "submit_follow_up"
    );
    fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
    let events = fixture
        .manager
        .list_events(&fixture.agent_id, 100, None)
        .await
        .unwrap();
    let values: Vec<Value> = events
        .iter()
        .filter_map(|entry| serde_json::from_str(&entry.message).ok())
        .collect();
    assert_eq!(
        values
            .iter()
            .filter(|value| value["type"] == "user_message")
            .count(),
        2
    );
    let receipts: Vec<_> = values
        .iter()
        .filter(|value| value["type"] == "input_receipt")
        .collect();
    assert_eq!(receipts.len(), 2);
    assert!(
        receipts
            .iter()
            .any(|value| value["message_id"] == "second" && value["receipt"]["status"] == "queued")
    );
    fixture.finish().await;
}

#[tokio::test]
async fn rejected_and_disconnected_inputs_keep_outcomes_without_resending() {
    for (scenario, status) in [
        ("input_reject", "rejected"),
        ("input_drop", "outcome_unknown"),
    ] {
        let fixture = Fixture::new(scenario).await;
        let session = fixture
            .manager
            .start_agent(&fixture.agent_id)
            .await
            .unwrap();
        let error = fixture
            .manager
            .send_input(
                &fixture.agent_id,
                "private-input",
                Some("input"),
                Some(&session),
            )
            .await
            .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<AgentSendInputError>(),
            Some(AgentSendInputError::NativeInputNotAccepted { .. })
        ));
        let receipt = fixture.input_receipt("input").await;
        assert_eq!(receipt.0, status);
        assert!(
            !receipt
                .1
                .unwrap_or_default()
                .contains("private-diagnostic-token")
        );
        assert_eq!(fixture.input_requests().len(), 1);
        if scenario == "input_drop" {
            fixture.assert_clean().await;
        } else {
            fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
        }
        fixture.finish().await;
    }
}

#[tokio::test]
async fn question_answers_cannot_retarget_a_new_pending_turn() {
    let fixture = Fixture::new("input_question").await;
    let session = fixture
        .manager
        .start_agent(&fixture.agent_id)
        .await
        .unwrap();
    let mut receiver = fixture
        .manager
        .subscribe_output(&fixture.agent_id)
        .await
        .unwrap();
    fixture
        .manager
        .send_input(&fixture.agent_id, "ask", Some("prompt"), Some(&session))
        .await
        .unwrap();
    let question = output(&mut receiver, "tool_call").await;
    let target: agenthub_rara::InputTarget =
        serde_json::from_value(question["meta"]["native_input"].clone()).unwrap();
    let missing = fixture
        .manager
        .send_input(&fixture.agent_id, "answer", Some("missing"), Some(&session))
        .await
        .unwrap_err();
    assert!(matches!(
        missing.downcast_ref::<AgentSendInputError>(),
        Some(AgentSendInputError::NativeInputRequired)
    ));
    fixture
        .manager
        .send_input_with_native_target(
            &fixture.agent_id,
            "alpha",
            &[],
            Some("answer"),
            Some(&session),
            Some(&target),
        )
        .await
        .unwrap();
    let next = output(&mut receiver, "tool_call").await;
    assert_ne!(next["meta"]["native_input"]["turn_id"], target.turn_id);
    let stale = fixture
        .manager
        .send_input_with_native_target(
            &fixture.agent_id,
            "stale answer",
            &[],
            Some("stale"),
            Some(&session),
            Some(&target),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        stale.downcast_ref::<AgentSendInputError>(),
        Some(AgentSendInputError::NativeInputMismatch)
    ));
    let requests = fixture.input_requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1]["payload"]["expected_turn_id"], "turn-1");
    assert_eq!(
        requests[1]["payload"]["envelope"]["request"]["payload"]["type"],
        "answer_pending_input"
    );
    fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
async fn caller_disconnect_does_not_abandon_the_receipt_owner() {
    let fixture = Fixture::new("input_delayed").await;
    let session = fixture
        .manager
        .start_agent(&fixture.agent_id)
        .await
        .unwrap();
    let manager = fixture.manager.clone();
    let agent = fixture.agent_id.clone();
    let request = tokio::spawn(async move {
        manager
            .send_input(&agent, "input", Some("detached"), Some(&session))
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        while fixture.input_requests().is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    std::fs::write(fixture.directory.join("release-ack"), "continue").unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while fixture.input_receipt("detached").await.0 != "accepted" {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(fixture.input_requests().len(), 1);
    fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
    fixture.finish().await;
}
