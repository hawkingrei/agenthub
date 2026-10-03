use agenthub_rara::InputTarget;

use super::{input::output, *};
use crate::agent::AgentSendInputError;

async fn answer(
    fixture: &Fixture,
    session: &str,
    target: &InputTarget,
    id: &str,
) -> anyhow::Result<()> {
    fixture
        .manager
        .send_input_with_native_target(
            &fixture.agent_id,
            "alpha",
            &[],
            Some(id),
            Some(session),
            Some(target),
        )
        .await
}

#[tokio::test]
async fn accepted_question_answer_fences_duplicates_before_events_commit() {
    for scenario in [
        "input_question_no_cursor",
        "input_question_zero_cursor",
        "input_question_stale_cursor",
    ] {
        let fixture = Fixture::new(scenario).await;
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
        let target: InputTarget =
            serde_json::from_value(question["meta"]["native_input"].clone()).unwrap();
        answer(&fixture, &session, &target, "answer").await.unwrap();
        let accepted = fixture.input_receipt("answer").await;
        let untargeted = fixture
            .manager
            .send_input(
                &fixture.agent_id,
                "new work",
                Some("untargeted"),
                Some(&session),
            )
            .await;
        let runtime = fixture.runtime().await;
        let duplicate_target = target.clone();
        let duplicate = tokio::spawn(async move {
            runtime
                .send_input(
                    "duplicate",
                    Some("duplicate"),
                    Some(&duplicate_target),
                    None,
                )
                .await
        });
        let pool = fixture
            .manager
            .event_dbs
            .pool_for_agent(&fixture.agent_id)
            .await
            .unwrap();
        let duplicate_attempts = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let attempts: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM runtime_control_receipts WHERE request_id = 'duplicate'",
                )
                .fetch_one(&pool)
                .await
                .unwrap();
                if duplicate.is_finished() || attempts > 0 {
                    break attempts;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let cursor: i64 = sqlx::query_scalar("SELECT last_sequence FROM runtime_event_streams")
            .fetch_one(&pool)
            .await
            .unwrap();
        std::fs::write(fixture.directory.join("release-question-events"), "ready").unwrap();
        let duplicate = tokio::time::timeout(Duration::from_secs(5), duplicate)
            .await
            .unwrap()
            .unwrap();
        let next = output(&mut receiver, "tool_call").await;
        let next_target: InputTarget =
            serde_json::from_value(next["meta"]["native_input"].clone()).unwrap();
        // Let the pre-fix duplicate finish before cleanup; it must not leak a provider.
        let next_answer = if duplicate.is_err() {
            answer(&fixture, &session, &next_target, "next-answer").await
        } else {
            Ok(())
        };
        let requests = fixture.input_requests();
        fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
        fixture.finish().await;

        assert_eq!(accepted.0, "accepted");
        assert_eq!(cursor, 3, "an answer ACK cannot advance committed history");
        assert!(matches!(
            untargeted
                .unwrap_err()
                .downcast_ref::<AgentSendInputError>(),
            Some(AgentSendInputError::NativeInputRequired)
        ));
        assert!(matches!(
            duplicate.unwrap_err().downcast_ref::<AgentSendInputError>(),
            Some(AgentSendInputError::NativeInputMismatch)
        ));
        assert_eq!(duplicate_attempts, 0);
        assert_ne!(next_target.turn_id, target.turn_id);
        next_answer.unwrap();
        let ids: Vec<_> = requests
            .iter()
            .map(|request| {
                request["payload"]["envelope"]["request_id"]
                    .as_str()
                    .unwrap()
            })
            .collect();
        assert_eq!(ids, ["prompt", "answer", "next-answer"]);
    }
}

#[tokio::test]
async fn rejected_question_answer_allows_an_explicit_retry() {
    let fixture = Fixture::new("input_question_reject_once").await;
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
    let target = serde_json::from_value(question["meta"]["native_input"].clone()).unwrap();
    let rejected = answer(&fixture, &session, &target, "rejected").await;
    let retry = answer(&fixture, &session, &target, "retry").await;
    let status = fixture.input_receipt("rejected").await.0;
    let requests = fixture.input_requests();
    fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
    fixture.finish().await;
    assert!(matches!(
        rejected.unwrap_err().downcast_ref::<AgentSendInputError>(),
        Some(AgentSendInputError::NativeInputNotAccepted { .. })
    ));
    assert_eq!(status, "rejected");
    retry.unwrap();
    assert_eq!(requests.len(), 3);
}
