use super::*;
use agenthub_agent_domain::loop_runtime::{LoopOutcomeKind, LoopWaitReason};

pub(super) fn is_guard_request(request: &Value) -> bool {
    request["messages"].as_array().is_some_and(|messages| {
        messages.iter().any(|message| {
            message["role"] == "system"
                && message["content"]
                    .as_str()
                    .is_some_and(|text| text.starts_with("Classify whether the supplied work fits"))
        })
    })
}

pub(super) fn compatible_response(
    request: &Value,
) -> Option<([(&'static str, &'static str); 1], String)> {
    is_guard_request(request).then(|| response(request, r#"{"outcome":"compatible"}"#))
}

fn response(request: &Value, content: &str) -> ([(&'static str, &'static str); 1], String) {
    let message = json!({"role":"assistant","content":content});
    let usage = json!({"prompt_tokens":10,"completion_tokens":10,"total_tokens":20});
    if request["stream"] == true {
        let chunk = json!({"id":"fixture","object":"chat.completion.chunk","model":"fixture-model","choices":[{"index":0,"delta":message,"finish_reason":"stop"}],"usage":usage});
        (
            [("content-type", "text/event-stream")],
            format!("data: {chunk}\n\ndata: [DONE]\n\n"),
        )
    } else {
        let body = json!({"id":"fixture","object":"chat.completion","model":"fixture-model","choices":[{"index":0,"message":message,"finish_reason":"stop"}],"usage":usage});
        ([("content-type", "application/json")], body.to_string())
    }
}

#[tokio::test]
#[ignore = "requires AGENTHUB_RARA_TEST_BINARY built from PINNED_UPSTREAM_REVISION"]
async fn native_loop_process_semantic_guard_declines_and_fallbacks_are_durable() {
    for (decision, expected) in [
        (
            json!({"outcome":"mismatch","reason":"Outside this role"}),
            Some(LoopOutcomeKind::NoActionableWork),
        ),
        (
            json!({"outcome":"needs_clarification","reason":"Missing target","question":"Which target?"}),
            Some(LoopOutcomeKind::Waiting),
        ),
        (json!({"outcome":"compatible"}), None),
        (
            json!({"outcome":"compatible","unexpected":"reject this field"}),
            None,
        ),
    ] {
        let mut fixture = Fixture::new("no-outcome").await;
        let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
        let app = Router::new().route(
            "/v1/chat/completions",
            post({
                let requests = requests.clone();
                move |Json(request): Json<Value>| {
                    let requests = requests.clone();
                    let decision = decision.to_string();
                    async move {
                        requests.lock().await.push(request.clone());
                        response(
                            &request,
                            if is_guard_request(&request) {
                                &decision
                            } else {
                                "Worker completed without a canonical finish."
                            },
                        )
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let state = fixture.directory.join("native-state");
        std::fs::create_dir(&state).unwrap();
        std::fs::write(state.join("config.json"), json!({"provider":"deepseek","api_key":"fixture-key","model":"fixture-model","base_url":format!("http://{address}/v1")}).to_string()).unwrap();
        std::fs::write(fixture.directory.join("native-settings.json"), json!({"binary":std::env::var("AGENTHUB_RARA_TEST_BINARY").expect("pinned binary path")}).to_string()).unwrap();
        let wrapper = fixture.directory.join("native-runtime");
        std::fs::write(&wrapper, WRAPPER).unwrap();
        std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut config = (*fixture.state.agents.loop_app_config).clone();
        config.rara = Some(agenthub_config::RaraConfig {
            binary: Some(wrapper.to_string_lossy().into_owned()),
            ..Default::default()
        });
        fixture.state.agents =
            Arc::new((*fixture.state.agents).clone().with_loop_app_config(config));
        sqlx::query("UPDATE agents SET command = 'rara', args = '[]', runtime_model = 'fixture-model' WHERE id = 'worker'").execute(&fixture.state.db).await.unwrap();
        let activation = fixture.execute("real-guard").await;
        if let Some(expected) = expected {
            assert_eq!(activation.state, LoopActivationState::Finished);
            let outcome = activation.outcome.as_ref().unwrap();
            assert_eq!(outcome.kind, expected);
            assert_eq!(
                outcome.wait_reason,
                (expected == LoopOutcomeKind::Waiting).then_some(LoopWaitReason::Input)
            );
        } else {
            assert_eq!(activation.state, LoopActivationState::Interrupted);
            assert!(activation.outcome.is_none());
        }
        let requests = requests.lock().await;
        assert!(is_guard_request(&requests[0]));
        assert!(requests[0].get("tools").is_none());
        let input = requests[0]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|message| message["role"] == "user")
            .unwrap()["content"]
            .as_str()
            .unwrap();
        let input: Value = serde_json::from_str(input).unwrap();
        assert_eq!(input["context"]["role"], "worker");
        assert!(
            input["context"]["work"]
                .as_str()
                .unwrap()
                .contains(&activation.id)
        );
        assert_eq!(
            requests
                .iter()
                .filter(|request| !is_guard_request(request))
                .count(),
            usize::from(expected.is_none())
        );
        drop(requests);
        let registrations: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM loop_registrations WHERE actor_id = 'worker'")
                .fetch_one(&fixture.state.db)
                .await
                .unwrap();
        assert_eq!(
            registrations,
            i64::from(expected == Some(LoopOutcomeKind::Waiting))
        );
        fixture.close().await;
        server.abort();
    }
}
