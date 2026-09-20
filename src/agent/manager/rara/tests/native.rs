use std::sync::Arc;

use agent_client_protocol::schema::v1::{RequestPermissionOutcome, SelectedPermissionOutcome};
use axum::{Json, Router, routing::post};
use serde_json::json;
use tokio::sync::Mutex;

use super::*;

/// The real pinned child uses a local model fixture; no external provider is contacted.
#[tokio::test]
#[ignore = "requires AGENTHUB_RARA_TEST_BINARY built from PINNED_UPSTREAM_REVISION"]
async fn managed_native_question_and_shell_approval_round_trip() {
    for choice in ["once", "deny"] {
        let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
        let app = Router::new().route("/v1/chat/completions", post({
            let requests = requests.clone();
            move |Json(request): Json<Value>| {
                let requests = requests.clone();
                async move {
                    let mut requests = requests.lock().await;
                    let index = requests.len();
                    requests.push(request.clone());
                    let (message, finish) = scripted_response(index);
                    let usage = json!({"prompt_tokens":10,"completion_tokens":10,"total_tokens":20});
                    if request["stream"] == true {
                        let chunk = json!({"id":"fixture","object":"chat.completion.chunk","model":"fixture-model","choices":[{"index":0,"delta":message,"finish_reason":finish}],"usage":usage});
                        ([("content-type", "text/event-stream")], format!("data: {chunk}\n\ndata: [DONE]\n\n"))
                    } else {
                        let body = json!({"id":"fixture","object":"chat.completion","model":"fixture-model","choices":[{"index":0,"message":message,"finish_reason":finish}],"usage":usage});
                        ([("content-type", "application/json")], body.to_string())
                    }
                }
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let cancellation = tokio_util::sync::CancellationToken::new();
        let stopped = cancellation.clone();
        let server = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(stopped.cancelled_owned())
                .await
                .unwrap();
        });
        let mut fixture = Fixture::new("normal").await;
        let native_state = fixture.directory.join("native-state");
        std::fs::create_dir(&native_state).unwrap();
        std::fs::write(
            native_state.join("config.json"),
            serde_json::to_vec(&json!({
                "provider":"deepseek", "api_key":"fixture-key", "model":"fixture-model",
                "base_url":format!("http://{address}/v1")
            }))
            .unwrap(),
        )
        .unwrap();
        fixture.manager.local_executor = Arc::new(NativeFixtureEnvironment {
            delegate: fixture.manager.local_executor.clone(),
            state: native_state,
        });
        fixture.manager = fixture
            .manager
            .with_loop_app_config(agenthub_config::AppConfig {
                rara: Some(agenthub_config::RaraConfig {
                    binary: Some(
                        std::env::var("AGENTHUB_RARA_TEST_BINARY").expect("pinned binary path"),
                    ),
                    ..Default::default()
                }),
                ..Default::default()
            });
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
            .send_input(
                &fixture.agent_id,
                "Start the local fixture",
                Some("prompt"),
                Some(&session),
            )
            .await
            .unwrap();
        let question = loop {
            let card = input::output(&mut receiver, "tool_call").await;
            if card["meta"]["native_input"].is_object() {
                break card;
            }
        };
        let target: agenthub_rara::InputTarget =
            serde_json::from_value(question["meta"]["native_input"].clone()).unwrap();
        fixture
            .manager
            .send_input_with_native_target(
                &fixture.agent_id,
                "Minimal",
                &[],
                Some("answer"),
                Some(&session),
                Some(&target),
            )
            .await
            .unwrap();
        let plan = input::output(&mut receiver, "permission_request").await;
        assert!(
            plan["options"]
                .as_array()
                .unwrap()
                .iter()
                .any(|option| option["option_id"] == "approve")
        );
        fixture
            .manager
            .permissions
            .respond(
                plan["permission_id"].as_str().unwrap(),
                RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new("approve")),
                Some("approve".into()),
                Some("fixture-operator".into()),
            )
            .await
            .unwrap();
        let permission = input::output(&mut receiver, "permission_request").await;
        let permission_id = permission["permission_id"].as_str().unwrap();
        assert!(!fixture.directory.join("approved-only").exists());
        fixture
            .manager
            .permissions
            .respond(
                permission_id,
                RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(choice)),
                Some(choice.into()),
                Some("fixture-operator".into()),
            )
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut acknowledged = false;
            let mut completed = false;
            while !acknowledged || !completed {
                let output = receiver.recv().await.unwrap();
                let Ok(event) = serde_json::from_str::<Value>(&output.message) else {
                    continue;
                };
                if event["type"] == "permission_control_receipt"
                    && event["permission_id"] == permission_id
                {
                    assert_eq!(event["receipt"]["status"], "accepted");
                    acknowledged = true;
                }
                if event["type"] == "agent_message"
                    && event["text"]
                        .as_str()
                        .is_some_and(|text| text.contains("Native fixture complete"))
                {
                    completed = true;
                }
            }
        })
        .await
        .unwrap();
        fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
        fixture.assert_clean().await;
        assert_eq!(
            fixture.directory.join("approved-only").exists(),
            choice == "once"
        );
        let requests = requests.lock().await;
        assert_eq!(requests.len(), 5);
        assert!(requests[2]["messages"].to_string().contains("Minimal"));
        drop(requests);
        let history = fixture
            .manager
            .runtime_history(&fixture.agent_id, &session, 100, None)
            .await
            .unwrap()
            .unwrap();
        assert!(history.closed);
        assert!(history.streams[0].cursor.sequence > 10);
        assert!(history.streams[0].cursor.gap.is_none());
        assert!(
            history.receipts.iter().all(|receipt| receipt.status
                == agenthub_db::runtime_events::RuntimeRequestStatus::Accepted)
        );
        let events = fixture
            .manager
            .list_events_for_session(&fixture.agent_id, &session, 100, None)
            .await
            .unwrap();
        let cards: Vec<Value> = events
            .iter()
            .filter_map(|event| serde_json::from_str(&event.message).ok())
            .collect();
        let tool_id = permission["tool_call_id"].as_str().unwrap();
        assert_eq!(
            cards
                .iter()
                .filter(|card| card["type"] == "tool_call_update"
                    && card["id"] == plan["tool_call_id"]
                    && card["status"] == "completed")
                .count(),
            1
        );
        assert_eq!(
            cards
                .iter()
                .filter(|card| card["type"] == "tool_call" && card["id"] == tool_id)
                .count(),
            1
        );
        assert_eq!(
            cards
                .iter()
                .filter(|card| card["type"] == "tool_call_update"
                    && card["id"] == tool_id
                    && matches!(card["status"].as_str(), Some("completed" | "failed")))
                .count(),
            1
        );
        fixture.finish().await;
        cancellation.cancel();
        server.await.unwrap();
    }
}

fn scripted_response(index: usize) -> (Value, &'static str) {
    let tool = |id: &str, name: &str, input: Value, content: Option<&str>| {
        (
            json!({"role":"assistant", "content":content, "tool_calls":[{
                "index":0, "id":id, "type":"function",
                "function":{"name":name, "arguments":input.to_string()}
            }]}),
            "tool_calls",
        )
    };
    match index {
        0 => tool("native-enter-plan", "enter_plan_mode", json!({}), None),
        1 => (
            json!({"role":"assistant", "content":concat!(
                "<request_user_input>\nquestion: Which path?\n",
                "option: Minimal | Keep the diff small.\noption: Broad | Expand.\n",
                "</request_user_input>\nChoose one."
            )}),
            "stop",
        ),
        2 => tool(
            "native-exit-plan",
            "exit_plan_mode",
            json!({}),
            Some(
                "<proposed_plan>\n- [pending] Create the fixture marker after approval\n</proposed_plan>",
            ),
        ),
        3 => tool(
            "native-shell-call",
            "bash",
            json!({
                "command":"printf fixture > approved-only",
                "sandbox_permissions":"require_escalated",
                "justification":"Native approval fixture", "prefix_rule":["printf"]
            }),
            None,
        ),
        _ => (
            json!({"role":"assistant", "content":"Native fixture complete"}),
            "stop",
        ),
    }
}
