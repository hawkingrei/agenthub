use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use super::*;

async fn question(fixture: &Fixture, previous_turn: Option<&str>) -> (String, Value) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let events = fixture
                .state
                .agents
                .list_events("worker", 100, None)
                .await
                .unwrap();
            for event in events.into_iter().rev() {
                let Ok(message) = serde_json::from_str::<Value>(&event.message) else {
                    continue;
                };
                let target = &message["meta"]["native_input"];
                if message["type"] == "tool_call"
                    && target["turn_id"]
                        .as_str()
                        .is_some_and(|turn| Some(turn) != previous_turn)
                {
                    return (event.session_id, target.clone());
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}

async fn answer(
    app: &axum::Router,
    token: &str,
    session: &str,
    id: &str,
    target: Option<Value>,
) -> StatusCode {
    let payload = json!({
        "input":"Use staging", "message_id":id, "session_id":session, "native_input":target
    });
    let request = Request::post("/agents/worker/input")
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    app.clone().oneshot(request).await.unwrap().status()
}

#[tokio::test]
async fn native_loop_question_api_accepts_only_current_fenced_answers() {
    let fixture = fixture("waiting").await;
    let token = crate::api::team_tests::create_auth_token(&fixture.state).await;
    let app = crate::api::router(fixture.state.clone());
    let (activation, (untargeted, invalid, valid, stale)) =
        tokio::join!(fixture.execute("question-api"), async {
            let (session, target) = question(&fixture, None).await;
            assert!(fixture.state.agents.has_loop_activation("worker").await);
            let untargeted = answer(&app, &token, &session, "untargeted", None).await;
            let mut invalid = Vec::new();
            for field in ["runtime_id", "session_id", "turn_id"] {
                let mut other = target.clone();
                other[field] = json!("wrong-owner");
                invalid.push(answer(&app, &token, &session, field, Some(other)).await);
            }
            invalid.push(
                answer(
                    &app,
                    &token,
                    "old-local-session",
                    "old-local",
                    Some(target.clone()),
                )
                .await,
            );
            let valid = answer(&app, &token, &session, "answer", Some(target.clone())).await;
            let stale = if valid == StatusCode::OK {
                question(&fixture, target["turn_id"].as_str()).await;
                Some(answer(&app, &token, &session, "old-turn", Some(target)).await)
            } else {
                None
            };
            fixture.state.agents.cancel_acp("worker").await.unwrap();
            (untargeted, invalid, valid, stale)
        });
    let log = std::fs::read_to_string(fixture.directory.join("native-requests.jsonl")).unwrap();
    fixture.close().await;
    assert!(
        !untargeted.is_success(),
        "ordinary input must use durable intake"
    );
    assert_eq!(
        valid,
        StatusCode::OK,
        "the active loop must accept its fenced question answer"
    );
    assert!(invalid.iter().all(|status| *status == StatusCode::CONFLICT));
    assert_eq!(stale, Some(StatusCode::CONFLICT));
    assert_eq!(log.matches("answer_pending_input").count(), 1);
    assert_eq!(log.matches("submit_user_prompt").count(), 1);
    assert_eq!(activation.state, LoopActivationState::Interrupted);
    assert!(activation.outcome.is_none());
}
