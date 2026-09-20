use agent_client_protocol::schema::v1::{RequestPermissionOutcome, SelectedPermissionOutcome};
use agenthub_agent_domain::loop_runtime::LoopActivation;
use agenthub_db::runtime_events::{RuntimeRequestKind, RuntimeRequestStatus};
use axum::{Json, Router, routing::post};
use serde_json::{Value, json};
use tokio::sync::Mutex;

use super::*;

mod cycle;

const CHILD_INSTRUCTION: &str =
    "native-loop-child-isolation: create a native private task and report";

const WRAPPER: &str = r#"#!/usr/bin/env python3
import json, os, pathlib, sys
root = pathlib.Path(__file__).parent
settings = json.loads((root / 'native-settings.json').read_text())
os.environ['RARA_HOME'] = str(root / 'native-state')
os.execv(settings['binary'], [settings['binary'], *sys.argv[1:]])
"#;

/// Runs the pinned native process against a local model and the real signed control CLI.
#[tokio::test]
#[ignore = "requires AGENTHUB_RARA_TEST_BINARY built from PINNED_UPSTREAM_REVISION"]
async fn native_loop_process_dispatch_report_and_acceptance_survive_each_exit() {
    let mut fixture = Fixture::new("no-outcome").await;
    let script = fixture.directory.join("native-cycle.py");
    let command = format!(
        "python3 '{}'",
        script.to_string_lossy().replace('\'', "'\\''")
    );
    let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
    let app = Router::new().route(
        "/v1/chat/completions",
        post({
            let requests = requests.clone();
            move |Json(request): Json<Value>| {
                let requests = requests.clone();
                let command = command.clone();
                async move {
                    requests.lock().await.push(request.clone());
                    model_response(&request, &command)
                }
            }
        }),
    );
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
    let native_state = fixture.directory.join("native-state");
    std::fs::create_dir(&native_state).unwrap();
    std::fs::write(
        native_state.join("config.json"),
        json!({"provider":"deepseek", "api_key":"fixture-key", "model":"fixture-model",
            "base_url":format!("http://{address}/v1")})
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        fixture.directory.join("native-settings.json"),
        json!({
            "binary":std::env::var("AGENTHUB_RARA_TEST_BINARY").expect("pinned binary path"),
            "control":crate::agenthub_binary::resolve_agenthub_binary_path().unwrap(),
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(fixture.directory.join("native-cycle.py"), cycle::SCRIPT).unwrap();
    let wrapper = fixture.directory.join("native-runtime");
    std::fs::write(&wrapper, WRAPPER).unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut config = (*fixture.state.agents.loop_app_config).clone();
    config.rara = Some(agenthub_config::RaraConfig {
        binary: Some(wrapper.to_string_lossy().into_owned()),
        ..Default::default()
    });
    fixture.state.agents = Arc::new((*fixture.state.agents).clone().with_loop_app_config(config));
    sqlx::query("UPDATE agents SET command = 'rara', args = '[]', runtime_model = 'fixture-model' WHERE id IN ('planner', 'worker')")
        .execute(&fixture.state.db).await.unwrap();
    let agent_ids: Vec<String> = sqlx::query_scalar("SELECT id FROM agents ORDER BY id")
        .fetch_all(&fixture.state.db)
        .await
        .unwrap();
    let store = LoopStore::new(fixture.state.db.clone());
    let now = Utc::now().timestamp();
    store
        .configure(
            LoopPolicyUpdate {
                actor_id: "planner",
                team_id: &fixture.team_id,
                expected_revision: 1,
                state: LoopPolicyState::Enabled,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &LoopLimits::default(),
            },
            now,
        )
        .await
        .unwrap();
    store
        .accept_trigger(
            &LoopTriggerInput {
                actor_id: "planner".into(),
                team_id: fixture.team_id.clone(),
                kind: LoopTriggerKind::Operator,
                source_key: "native-dispatch".into(),
                due_at: None,
                references: LoopSourceReferences::default(),
            },
            now,
        )
        .await
        .unwrap();

    let first = execute_pending(&fixture, "planner").await;
    let worker = execute_pending(&fixture, "worker").await;
    let last = execute_pending(&fixture, "planner").await;
    assert_ne!(first.session_id, last.session_id);
    assert_eq!(first.mailbox_run_id, last.mailbox_run_id);
    for (activation, previous, count) in [(&worker, &first, 2), (&last, &worker, 1)] {
        let sources = store
            .triggers(&fixture.team_id, &activation.id)
            .await
            .unwrap();
        assert_eq!(sources.len(), count);
        assert!(sources.iter().all(|source| {
            source.input.references.scheduling_activation_id.as_deref()
                == Some(previous.id.as_str())
        }));
    }
    let status: String =
        sqlx::query_scalar("SELECT status FROM team_tasks WHERE title = 'Native offline review'")
            .fetch_one(&fixture.state.db)
            .await
            .unwrap();
    assert_eq!(status, "completed");
    let tasks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM team_tasks WHERE team_id = ?")
        .bind(&fixture.team_id)
        .fetch_one(&fixture.state.db)
        .await
        .unwrap();
    assert_eq!(tasks, 1, "native child tasks are not canonical Team tasks");
    let remaining_agents: Vec<String> = sqlx::query_scalar("SELECT id FROM agents ORDER BY id")
        .fetch_all(&fixture.state.db)
        .await
        .unwrap();
    assert_eq!(
        agent_ids, remaining_agents,
        "native children gain no outer identity"
    );
    let pending: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM loop_activations WHERE state = 'pending'")
            .fetch_one(&fixture.state.db)
            .await
            .unwrap();
    assert_eq!(pending, 0);
    let transcript: Vec<Value> =
        std::fs::read_to_string(fixture.directory.join("native-cycle.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
    assert_eq!(transcript.len(), 3);
    assert_eq!(transcript[0]["stage"], "dispatch");
    assert_eq!(transcript[1]["stage"], "report");
    assert_eq!(transcript[2]["stage"], "accept");
    assert!(
        transcript
            .iter()
            .all(|entry| entry["child_identity_denied"] == true)
    );
    assert!(
        fixture
            .state
            .agents
            .loop_credentials
            .lock()
            .await
            .is_empty()
    );

    let requests = requests.lock().await;
    let child_results: Vec<_> = requests
        .iter()
        .filter(|request| {
            is_child_request(request)
                && request["messages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|message| {
                        message["role"] == "tool"
                            && message["tool_call_id"] == "native-private-task"
                    })
        })
        .collect();
    assert_eq!(
        child_results.len(),
        3,
        "each native child executed its own task tool"
    );
    for request in child_results {
        let result = request["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|message| {
                message["role"] == "tool" && message["tool_call_id"] == "native-private-task"
            })
            .unwrap();
        assert!(
            result["content"]
                .to_string()
                .contains("Native private task"),
            "{result}"
        );
    }
    for (activation, role) in [
        (&first, "Team Coordinator"),
        (&worker, "Team Worker"),
        (&last, "Team Coordinator"),
    ] {
        let request = requests
            .iter()
            .find(|request| request["messages"].to_string().contains(&activation.id))
            .expect("the native model receives the registered activation source");
        let messages = request["messages"].to_string();
        assert!(messages.contains(role));
        assert_eq!(
            messages
                .matches("Run one bounded AgentHub activation.")
                .count(),
            1
        );
    }
    drop(requests);
    fixture.close().await;
    cancellation.cancel();
    server.await.unwrap();
}

async fn execute_pending(fixture: &Fixture, actor: &str) -> LoopActivation {
    let store = LoopStore::new(fixture.state.db.clone());
    let id: String = sqlx::query_scalar("SELECT id FROM loop_activations WHERE actor_id = ? AND state = 'pending' ORDER BY id LIMIT 1")
        .bind(actor).fetch_one(&fixture.state.db).await.unwrap();
    let LoopAdmission::Admitted(reservation) = store
        .admit(
            &fixture.team_id,
            &id,
            fixture.state.agents.loop_owner_id(),
            Utc::now().timestamp(),
        )
        .await
        .unwrap()
    else {
        panic!("not admitted");
    };
    fixture
        .state
        .agents
        .track_loop_reservation(reservation.clone())
        .await
        .unwrap();
    let execution = fixture
        .state
        .agents
        .execute_loop_activation(fixture.state.teams.clone(), reservation);
    tokio::pin!(execution);
    let mut ticker = tokio::time::interval(Duration::from_millis(20));
    let mut approvals = 0;
    tokio::time::timeout(Duration::from_secs(60), async {
        // Poll both futures while database calls wait: execution may own a transaction.
        let approvals_pending = async {
            loop {
                ticker.tick().await;
                let permission: Option<String> = sqlx::query_scalar(
                    "SELECT p.id FROM acp_permission_requests p JOIN loop_activations a ON a.session_id = p.session_id WHERE a.id = ? AND p.agent_id = ? AND p.status = 'pending'",
                ).bind(&id).bind(actor).fetch_optional(&fixture.state.db).await.unwrap();
                if let Some(permission) = permission {
                    fixture.state.agents.permissions.respond(
                        &permission,
                        RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new("once")),
                        Some("once".into()), Some("fixture-operator".into()),
                    ).await.unwrap();
                    approvals += 1;
                }
            }
        };
        tokio::select! {
            result = &mut execution => result.unwrap(),
            _ = approvals_pending => unreachable!("approval polling ends with execution"),
        }
    }).await.expect("native activation must finish after the live approval");
    let activation = store
        .activation(&fixture.team_id, &id)
        .await
        .unwrap()
        .unwrap();
    if activation.state != LoopActivationState::Finished {
        for event in fixture
            .state
            .agents
            .list_events_for_session(actor, activation.session_id.as_deref().unwrap(), 100, None)
            .await
            .unwrap()
        {
            eprintln!("native fixture event: {}", event.message);
        }
    }
    assert_eq!(
        activation.state,
        LoopActivationState::Finished,
        "{activation:?}; fixture: {}",
        fixture.directory.display()
    );
    assert_eq!(approvals, 1);
    assert!(
        store
            .reservation(&fixture.team_id, actor)
            .await
            .unwrap()
            .is_none()
    );
    assert!(!fixture.state.agents.inner.read().await.contains_key(actor));
    let history = fixture
        .state
        .agents
        .runtime_history(actor, activation.session_id.as_deref().unwrap(), 100, None)
        .await
        .unwrap()
        .unwrap();
    assert!(history.closed);
    assert!(
        history
            .streams
            .iter()
            .all(|stream| stream.cursor.gap.is_none())
    );
    assert_eq!(
        history
            .receipts
            .iter()
            .filter(|receipt| receipt.kind == RuntimeRequestKind::Prompt)
            .count(),
        1
    );
    assert!(
        history
            .receipts
            .iter()
            .any(|receipt| receipt.kind == RuntimeRequestKind::ShellAnswer
                && receipt.status == RuntimeRequestStatus::Accepted)
    );
    activation
}

fn model_response(request: &Value, command: &str) -> ([(&'static str, &'static str); 1], String) {
    let completed = |id| {
        request["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|message| message["role"] == "tool" && message["tool_call_id"] == id)
    };
    let tool = |id, name, input: Value| {
        (
            json!({"role":"assistant", "content":null, "tool_calls":[{
                "index":0, "id":id, "type":"function",
                "function":{"name":name, "arguments":input.to_string()}
            }]}),
            "tool_calls",
        )
    };
    let (message, finish) = if is_child_request(request) {
        if completed("native-private-task") {
            (
                json!({"role":"assistant", "content":"Native child task created"}),
                "stop",
            )
        } else {
            tool(
                "native-private-task",
                "task_create",
                json!({
                    "subject":"Native private task", "description":"Local execution detail only"
                }),
            )
        }
    } else if !completed("native-loop-child") {
        tool(
            "native-loop-child",
            "spawn_agent",
            json!({
                "name":"general", "instruction":CHILD_INSTRUCTION, "run_in_background":false
            }),
        )
    } else if completed("native-loop-control") {
        (
            json!({"role":"assistant", "content":"Native loop fixture complete"}),
            "stop",
        )
    } else {
        (
            json!({"role":"assistant", "content":null, "tool_calls":[{
            "index":0, "id":"native-loop-control", "type":"function",
            "function":{"name":"bash", "arguments":json!({
                "command":command, "sandbox_permissions":"require_escalated",
                    "justification":"Execute the local loop fixture", "prefix_rule":["python3", "native-cycle.py"]
                }).to_string()}
            }]}),
            "tool_calls",
        )
    };
    let usage = json!({"prompt_tokens":10,"completion_tokens":10,"total_tokens":20});
    if request["stream"] == true {
        let chunk = json!({"id":"fixture","object":"chat.completion.chunk","model":"fixture-model","choices":[{"index":0,"delta":message,"finish_reason":finish}],"usage":usage});
        (
            [("content-type", "text/event-stream")],
            format!("data: {chunk}\n\ndata: [DONE]\n\n"),
        )
    } else {
        let body = json!({"id":"fixture","object":"chat.completion","model":"fixture-model","choices":[{"index":0,"message":message,"finish_reason":finish}],"usage":usage});
        ([("content-type", "application/json")], body.to_string())
    }
}

fn is_child_request(request: &Value) -> bool {
    request["messages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|message| {
            message["role"] == "user" && message["content"].to_string().contains(CHILD_INSTRUCTION)
        })
}
