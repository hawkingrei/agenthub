//! Opt-in acceptance with the installed ACP adapter and official Codex app-server.
//! Only the Responses model is scripted; ACP, native commands and actor RPC are real.

use super::*;
use axum::{Json, extract::State, response::IntoResponse, routing::post};
use serde_json::{Value, json};
use std::sync::Mutex;

mod browser;
mod tools;

const RELAY: &str = r#"#!/usr/bin/env python3
import json, os, subprocess, sys, threading
environment = dict(os.environ)
environment['CODEX_HOME'] = __PROFILE__
assert 'TEST_MEM_UPSTREAM_KEY' not in environment
assert 'TEST_APP_TOKEN' not in environment
assert 'TEST_EVENT_KEY' not in environment
child = subprocess.Popen([__DAEMON__] + sys.argv[1:], env=environment, stdin=subprocess.PIPE, stdout=subprocess.PIPE)
def forward():
    for line in sys.stdin.buffer:
        message = json.loads(line)
        with open(__LOG__, 'a') as output:
            record = {'method': message.get('method')}
            if message.get('method') == 'session/prompt':
                prompt = json.dumps(message['params']['prompt'])
                assert '<name>team-loop-runtime</name>' in prompt
                record['context_ready'] = 'Attributed DATA: acceptance scope space-a' in prompt
            output.write(json.dumps(record) + '\n')
        try:
            child.stdin.write(line)
            child.stdin.flush()
        except BrokenPipeError:
            return
    child.stdin.close()
threading.Thread(target=forward, daemon=True).start()
for line in child.stdout:
    sys.stdout.buffer.write(line)
    sys.stdout.buffer.flush()
sys.exit(child.wait())
"#;

const WORKFLOW: &str = r#"import json, os, subprocess, sys
control, stage = sys.argv[1:]
def actor(*args):
    result = subprocess.run([control, 'actor', *args, '--json'], capture_output=True, text=True)
    assert result.returncode == 0, result.stderr
    return json.loads(result.stdout)
page = actor('loop-context', '--limit', '1')
activation = page['activation']
sources = page['sources']
while page['next_cursor']:
    page = actor('loop-context', '--limit', '1', '--after-source-id', page['next_cursor'])
    sources.extend(page['sources'])
details = [actor('loop-source', '--source-id', source['id']) for source in sources]
if stage == 'read':
    print(json.dumps({'actor': activation['actor_id'], 'sources': len(details)}))
elif stage == 'work':
    tasks = actor('team-tasks')
    if activation['actor_id'] == 'planner' and not tasks:
        task = actor('team-task-create', '--title', 'Real ACP acceptance', '--priority', 'medium', '--assigned-member-id', 'worker')['task']
        actor('send', '--to', 'worker', '--text', 'Review real runtime work', '--idempotency-key', 'real-dispatch')
        note = actor('team-task-note', '--task-id', task['id'], '--kind', 'decision', '--text', 'Dispatched real runtime review to worker')
    elif activation['actor_id'] == 'worker':
        task = next(task for task in tasks if task['title'] == 'Real ACP acceptance')
        denied = subprocess.run([control, 'actor', 'team-task-update', '--team-id', activation['team_id'], '--task-id', task['id'], '--status', 'completed', '--note-kind', 'result', '--note', 'Worker cannot accept its own result', '--json'], capture_output=True, text=True)
        assert denied.returncode != 0 and 'coordinator' in denied.stderr.lower(), denied.stderr
        note = actor('team-task-note', '--task-id', task['id'], '--kind', 'result', '--text', 'Evidence from the actual ACP runtime')
        actor('send', '--to', 'planner', '--text', 'Real runtime evidence ready', '--idempotency-key', 'real-report')
    else:
        task = next(task for task in tasks if task['title'] == 'Real ACP acceptance')
        detail = actor('team-task-show', '--task-id', task['id'])
        assert any(note['from_actor_id'] == 'worker' and note['text'] == 'Evidence from the actual ACP runtime' for note in detail['notes'])
        if task['status'] == 'completed':
            note = actor('team-task-note', '--task-id', task['id'], '--kind', 'result', '--text', 'Independent local progress during Mem outage')
        else:
            actor('team-task-update', '--team-id', activation['team_id'], '--task-id', task['id'], '--status', 'completed', '--note-kind', 'decision', '--note', 'Accepted actual runtime evidence')
            detail = actor('team-task-show', '--task-id', task['id'])
            note = next(note for note in detail['notes'] if note['from_actor_id'] == 'planner' and note['text'] == 'Accepted actual runtime evidence')
    with open('real-outcome.json', 'w') as output:
        json.dump({'kind':'handoff', 'task_note_id':note['message_id']}, output)
    print('Canonical work recorded.')
elif stage == 'finish':
    actor('loop-finish', '--outcome-file', os.path.abspath('real-outcome.json'))
else:
    raise AssertionError(stage)
"#;

struct Model {
    command: String,
    requests: Mutex<Vec<(String, String)>>,
    db: sqlx::SqlitePool,
    team_id: String,
    app_id: String,
    directory: std::path::PathBuf,
}

async fn model_response(
    State(model): State<Arc<Model>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let input = body["input"].as_array().expect("Responses input");
    let last_user = input
        .iter()
        .rposition(|item| item["role"] == "user")
        .unwrap_or(0);
    let outputs = input[last_user..]
        .iter()
        .filter(|item| {
            item["type"] == "function_call_output" || item["type"] == "tool_search_output"
        })
        .collect::<Vec<_>>();
    let completed = |stage: &str| {
        outputs.iter().any(|item| {
            item["call_id"]
                .as_str()
                .is_some_and(|id| id.starts_with(&format!("acceptance-{stage}-")))
        })
    };
    for (stage, expected) in [
        ("foreign", "MCP call scope does not match its binding"),
        ("revoked", "tool call error"),
    ] {
        if let Some(output) = outputs.iter().find(|item| {
            item["call_id"]
                .as_str()
                .is_some_and(|id| id.starts_with(&format!("acceptance-{stage}-")))
        }) {
            assert!(
                output["output"].as_str().unwrap().contains(expected),
                "{stage}: {output}"
            );
        }
    }
    let worker = outputs.iter().any(|item| {
        item["output"]
            .as_str()
            .is_some_and(|output| output.contains("\"actor\": \"worker\""))
    });
    let mut names = Vec::new();
    let mut namespaces = std::collections::HashMap::new();
    let mut searchable = false;
    let discovered = outputs
        .iter()
        .filter(|item| item["type"] == "tool_search_output")
        .filter_map(|item| item["tools"].as_array())
        .flatten();
    for tool in body["tools"]
        .as_array()
        .expect("native tools")
        .iter()
        .chain(discovered)
    {
        if tool["type"] == "tool_search" {
            searchable = true;
        } else if tool["type"] == "namespace" {
            for function in tool["tools"].as_array().unwrap() {
                let name = function["name"].as_str().unwrap();
                names.push(name);
                namespaces.insert(name, tool["name"].as_str().unwrap());
            }
        } else if let Some(name) = tool["name"].as_str() {
            names.push(name);
        }
    }
    let stage = if completed("finish") {
        "done"
    } else if completed("work") {
        "finish"
    } else if worker && completed("revoked") {
        "work"
    } else if worker && completed("app") {
        "revoked"
    } else if completed("mem") {
        if worker { "app" } else { "work" }
    } else if completed("foreign") {
        "mem"
    } else if completed("discover") {
        "foreign"
    } else if completed("read") {
        if searchable { "discover" } else { "foreign" }
    } else {
        "read"
    };
    if stage == "read" {
        while model.directory.join("browser-hold").exists() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
    let native = ["exec_command", "shell_command", "shell"]
        .into_iter()
        .find(|name| names.contains(name))
        .expect("native command tool");
    let (tool, arguments) = match stage {
        "discover" => (
            "tool_search",
            json!({"query":"memory_search write", "limit":10}),
        ),
        "foreign" | "mem" => {
            let tool = names
                .iter()
                .find(|name| **name == "memory_search" || name.ends_with("__memory_search"))
                .unwrap_or_else(|| panic!("Mem tool missing: {names:?}"));
            (
                *tool,
                if stage == "foreign" {
                    json!({"query":"acceptance", "space_id":"space-b"})
                } else {
                    json!({"query":"acceptance"})
                },
            )
        }
        "app" | "revoked" => {
            let tool = names
                .iter()
                .find(|name| **name == "write" || name.ends_with("__write"))
                .unwrap_or_else(|| panic!("App tool missing: {names:?}"));
            if stage == "revoked" {
                tools::revoke(&model.db, &model.team_id, &model.app_id).await;
            }
            (*tool, json!({"body":"accepted write"}))
        }
        _ => {
            let command = format!("{} {stage}", model.command);
            (
                native,
                match native {
                    "exec_command" => json!({"cmd":command,"yield_time_ms":10000}),
                    "shell_command" => json!({"command":command,"timeout_ms":10000}),
                    _ => json!({"command":["/bin/sh","-c",command],"timeout_ms":10000}),
                },
            )
        }
    };
    let index = {
        let mut requests = model.requests.lock().unwrap();
        requests.push((stage.into(), tool.into()));
        requests.len()
    };
    let item = if stage == "done" {
        json!({"type":"message","role":"assistant","id":format!("message-{index}"),"content":[{"type":"output_text","text":"Activation finished."}]})
    } else if stage == "discover" {
        json!({"type":"tool_search_call","call_id":format!("acceptance-{stage}-{index}"),"execution":"client","arguments":arguments})
    } else {
        let mut item = json!({"type":"function_call","call_id":format!("acceptance-{stage}-{index}"),"name":tool,"arguments":arguments.to_string()});
        if let Some(namespace) = namespaces.get(tool) {
            item["namespace"] = json!(namespace);
        }
        item
    };
    let events = [
        json!({"type":"response.created","response":{"id":format!("response-{index}")}}),
        json!({"type":"response.output_item.done","item":item}),
        json!({"type":"response.completed","response":{"id":format!("response-{index}"),"usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}}}),
    ];
    let body = events
        .iter()
        .map(|event| {
            format!(
                "event: {}\ndata: {event}\n\n",
                event["type"].as_str().unwrap()
            )
        })
        .collect::<String>();
    (
        [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
        body,
    )
}

async fn execute(
    fixture: &Fixture,
    actor: &str,
) -> agenthub_agent_domain::loop_runtime::LoopActivation {
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
        panic!("not admitted")
    };
    fixture
        .state
        .agents
        .track_loop_reservation(reservation.clone())
        .await
        .unwrap();
    let result = tokio::time::timeout(
        Duration::from_secs(300),
        fixture
            .state
            .agents
            .execute_loop_activation(fixture.state.teams.clone(), reservation.clone()),
    )
    .await;
    if !matches!(result, Ok(Ok(()))) {
        fixture
            .state
            .agents
            .fence_loop_reservation(&reservation)
            .await
            .unwrap();
        panic!(
            "actual ACP execution failed: {result:?}; workspace: {}",
            fixture.directory.display()
        );
    }
    let activation = store
        .activation(&fixture.team_id, &id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        activation.state,
        LoopActivationState::Finished,
        "{activation:?}; workspace: {}",
        fixture.directory.display()
    );
    assert!(
        store
            .reservation(&fixture.team_id, actor)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store
            .policy(&fixture.team_id, actor)
            .await
            .unwrap()
            .unwrap()
            .no_progress_count,
        0,
        "each completed activation must reference its canonical progress evidence"
    );
    activation
}

#[tokio::test]
#[ignore = "requires built LOOP_REAL_ACP_BINARY and official LOOP_REAL_CODEX_BINARY (0.150.1)"]
async fn loop_real_acp_dispatch_worker_and_fresh_acceptance() {
    let daemon = std::fs::canonicalize(std::env::var("LOOP_REAL_ACP_BINARY").unwrap()).unwrap();
    let codex = std::fs::canonicalize(std::env::var("LOOP_REAL_CODEX_BINARY").unwrap()).unwrap();
    let database_dir =
        std::env::temp_dir().join(format!("agenthub-real-acp-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&database_dir).unwrap();
    let database = database_dir.join("control.sqlite");
    agenthub_db::init_db_at_path(&database)
        .await
        .unwrap()
        .close()
        .await;
    let state = crate::api::team_tests::reopen_test_state_with_db_path(&database).await;
    let (upstream_endpoint, upstream, upstream_server) = tools::serve().await;
    let fixture = Fixture::with_state(
        state,
        "real-runtime",
        Some(&format!("{upstream_endpoint}/mem/mcp")),
    )
    .await;
    let app_id = tools::register(&fixture, &upstream_endpoint).await;
    let control = crate::agenthub_binary::resolve_agenthub_binary_path().unwrap();
    let workflow = fixture.directory.join("real_workflow.py");
    std::fs::write(&workflow, WORKFLOW).unwrap();
    let model = Arc::new(Model {
        command: format!(
            "python3 '{}' '{}'",
            workflow.to_string_lossy().replace('\'', "'\\''"),
            control.to_string_lossy().replace('\'', "'\\''")
        ),
        requests: Mutex::new(Vec::new()),
        db: fixture.state.db.clone(),
        team_id: fixture.team_id.clone(),
        app_id,
        directory: fixture.directory.clone(),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let app = axum::Router::new()
        .route("/v1/responses", post(model_response))
        .with_state(model.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let profile = fixture.directory.join("runtime-profile");
    std::fs::create_dir(&profile).unwrap();
    let relay = RELAY
        .replace("__PROFILE__", &serde_json::to_string(&profile).unwrap())
        .replace("__DAEMON__", &serde_json::to_string(&daemon).unwrap())
        .replace(
            "__LOG__",
            &serde_json::to_string(&fixture.directory.join("requests.jsonl")).unwrap(),
        );
    let program = fixture.directory.join("agenthubd");
    std::fs::write(&program, relay).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut args = vec![
        "acp".into(),
        "codex".into(),
        "--codex-binary".into(),
        codex.to_string_lossy().into_owned(),
    ];
    let overrides = [
        "model=\"gpt-5.4-mini\"".into(),
        "model_provider=\"acceptance\"".into(),
        format!(
            "model_providers.acceptance={{name=\"Local acceptance\",base_url=\"{endpoint}\",wire_api=\"responses\",requires_openai_auth=false}}"
        ),
        "features.code_mode=false".into(),
        "features.code_mode_only=false".into(),
        "features.plugins=false".into(),
        "features.apps=false".into(),
        "features.memories=false".into(),
        "features.responses_websockets=false".into(),
        "features.responses_websockets_v2=false".into(),
    ];
    for value in overrides {
        args.extend(["-c".into(), value]);
    }
    sqlx::query("UPDATE agents SET command = ?, args = ?, runtime_model = 'gpt-5.4-mini', thinking_level = 'low', codex_acp_default_mode = 'full-access' WHERE id IN ('planner', 'worker')")
        .bind(program.to_string_lossy().as_ref()).bind(serde_json::to_string(&args).unwrap()).execute(&fixture.state.db).await.unwrap();
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
    let browser = browser::Browser::start(&fixture, &database).await;
    tools::signed_event(&fixture, &model.app_id).await;
    let first = execute(&fixture, "planner").await;
    let worker = execute(&fixture, "worker").await;
    let accepted = execute(&fixture, "planner").await;
    assert_ne!(first.session_id, accepted.session_id);
    assert_ne!(worker.session_id, accepted.session_id);
    assert_eq!(first.mailbox_run_id, accepted.mailbox_run_id);
    let task_status: String = sqlx::query_scalar(
        "SELECT status FROM team_tasks WHERE team_id = ? AND title = 'Real ACP acceptance'",
    )
    .bind(&fixture.team_id)
    .fetch_one(&fixture.state.db)
    .await
    .unwrap();
    assert_eq!(task_status, "completed");
    let requests = std::fs::read_to_string(fixture.directory.join("requests.jsonl")).unwrap();
    assert_eq!(requests.matches("session/new").count(), 3, "{requests}");
    assert_eq!(requests.matches("session/prompt").count(), 3, "{requests}");
    assert_eq!(requests.matches("session/load").count(), 0, "{requests}");
    for stage in ["read", "work", "finish"] {
        assert_eq!(
            model
                .requests
                .lock()
                .unwrap()
                .iter()
                .filter(|(actual, _)| actual == stage)
                .count(),
            3,
            "{stage}"
        );
    }
    println!(
        "Actual ACP acceptance: three fresh sessions, three entries, nine native tool rounds, durable task accepted; Codex 0.150.1"
    );
    assert_eq!(
        upstream.contexts.load(std::sync::atomic::Ordering::SeqCst),
        3
    );
    assert_eq!(
        upstream.searches.load(std::sync::atomic::Ordering::SeqCst),
        3
    );
    assert_eq!(upstream.writes.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(requests.matches("\"context_ready\": true").count(), 3);
    upstream
        .outage
        .store(true, std::sync::atomic::Ordering::SeqCst);
    store
        .accept_trigger(
            &LoopTriggerInput {
                actor_id: "planner".into(),
                team_id: fixture.team_id.clone(),
                kind: LoopTriggerKind::Operator,
                source_key: "mem-unavailable-local-progress".into(),
                due_at: None,
                references: LoopSourceReferences::default(),
            },
            Utc::now().timestamp(),
        )
        .await
        .unwrap();
    let unavailable = execute(&fixture, "planner").await;
    let kind: String = sqlx::query_scalar("SELECT kind FROM loop_activation_events WHERE activation_id = ? AND kind LIKE 'mem_context_%'")
        .bind(&unavailable.id).fetch_one(&fixture.state.db).await.unwrap();
    assert_eq!(kind, "mem_context_unavailable");
    assert_eq!(upstream.writes.load(std::sync::atomic::Ordering::SeqCst), 1);
    if let Some(browser) = browser {
        fixture
            .state
            .agents
            .spawn_loop_worker(fixture.state.teams.clone())
            .unwrap();
        browser.finish().await;
    }
    fixture.close().await;
    server.abort();
    upstream_server.abort();
    std::fs::remove_dir_all(database_dir).unwrap();
}
