use super::*;

mod browser;
#[cfg(target_os = "linux")]
mod configured;

const WRAPPER_WITH_PID: &str = r#"#!/usr/bin/env python3
import json, os, pathlib, sys
root = pathlib.Path(__file__).parent
settings = json.loads((root / 'native-settings.json').read_text())
os.environ['RARA_HOME'] = str(root / 'native-state')
(root / 'native-pid').write_text(str(os.getpid()))
os.execv(settings['binary'], [settings['binary'], *sys.argv[1:]])
"#;

struct NativeFixture {
    fixture: Fixture,
    requests: Arc<Mutex<Vec<Value>>>,
    server: tokio::task::JoinHandle<()>,
}

impl NativeFixture {
    async fn new(mode: &'static str) -> Self {
        let fixture = if mode == "uncertain"
            && let Some(directory) = browser::directory()
        {
            std::fs::create_dir_all(&directory).unwrap();
            let database = directory.join("control.sqlite");
            assert!(!database.exists(), "use a fresh isolated browser directory");
            let pool = agenthub_db::init_db_at_path(&database).await.unwrap();
            pool.close().await;
            let state = crate::api::team_tests::reopen_test_state_with_db_path(&database).await;
            Fixture::with_state(state, "no-outcome", None).await
        } else {
            Fixture::new("no-outcome").await
        };
        Self::with_fixture(mode, fixture).await
    }

    async fn with_fixture(mode: &'static str, mut fixture: Fixture) -> Self {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let app = Router::new().route(
            "/v1/chat/completions",
            post({
                let requests = requests.clone();
                move |Json(request): Json<Value>| {
                    let requests = requests.clone();
                    async move {
                        requests.lock().await.push(request.clone());
                        response(&request, mode)
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
        std::fs::write(&wrapper, WRAPPER_WITH_PID).unwrap();
        std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut config = (*fixture.state.agents.loop_app_config).clone();
        config.rara = Some(agenthub_config::RaraConfig {
            binary: Some(wrapper.to_string_lossy().into_owned()),
            ..Default::default()
        });
        fixture.state.agents =
            Arc::new((*fixture.state.agents).clone().with_loop_app_config(config));
        sqlx::query("UPDATE agents SET command = 'rara', args = '[]', runtime_model = 'fixture-model' WHERE id = 'worker'")
            .execute(&fixture.state.db).await.unwrap();
        fixture.session_policy(LoopSessionPolicy::Resume).await;
        Self {
            fixture,
            requests,
            server,
        }
    }

    async fn recovery(&self) -> crate::agent::manager::rara::NativeRecoveryView {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let runtime = {
                    let handles = self.fixture.state.agents.inner.read().await;
                    handles
                        .get("worker")
                        .and_then(|handle| match &handle.input {
                            AgentInput::Rara(runtime) => Some(runtime.clone()),
                            _ => None,
                        })
                };
                if let Some(runtime) = runtime
                    && let Ok(view) = runtime.query_recovery().await
                {
                    return view;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("current native recovery entry")
    }

    async fn pending(&self) -> (String, String) {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let row = sqlx::query_as("SELECT id, session_id FROM acp_permission_requests WHERE agent_id = 'worker' AND status = 'pending' ORDER BY created_at DESC LIMIT 1")
                    .fetch_optional(&self.fixture.state.db).await.unwrap();
                if let Some(row) = row { return row; }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.expect("native pending approval")
    }

    fn crash(&self) {
        let pid: u32 = std::fs::read_to_string(self.fixture.directory.join("native-pid"))
            .unwrap()
            .parse()
            .unwrap();
        let expected =
            std::fs::canonicalize(std::env::var("AGENTHUB_RARA_TEST_BINARY").unwrap()).unwrap();
        assert_eq!(
            std::fs::read_link(format!("/proc/{pid}/exe")).unwrap(),
            expected
        );
        assert!(
            std::process::Command::new("kill")
                .args(["-KILL", "--", &pid.to_string()])
                .status()
                .unwrap()
                .success()
        );
    }

    async fn answer(&self, id: &str, choice: &str) -> crate::acp::AcpPermissionRespondResult {
        self.fixture
            .state
            .agents
            .permissions
            .respond(
                id,
                RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                    choice.to_owned(),
                )),
                Some(choice.into()),
                Some("fixture-operator".into()),
            )
            .await
            .unwrap()
    }

    async fn close(self) {
        self.fixture.close().await;
        self.server.abort();
    }
}

fn response(request: &Value, mode: &str) -> ([(&'static str, &'static str); 1], String) {
    if let Some(response) = semantic_guard::compatible_response(request) {
        return response;
    }
    let completed = request["messages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|message| message["role"] == "tool" && message["tool_call_id"] == "resume-shell");
    let (message, finish) = if mode != "clean" && !completed {
        let command = if mode == "uncertain" {
            "printf 'executed\\n' >> native-effects; sleep 60"
        } else {
            "printf 'executed\\n' >> native-effects"
        };
        (
            json!({"role":"assistant","content":null,"tool_calls":[{"index":0,"id":"resume-shell","type":"function","function":{"name":"bash","arguments":json!({"command":command,"sandbox_permissions":"require_escalated","justification":"Verify restart approval ownership","prefix_rule":["printf"]}).to_string()}}]}),
            "tool_calls",
        )
    } else {
        (
            json!({"role":"assistant","content":"Persistent native continuity marker"}),
            "stop",
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

async fn assert_new_owner(fixture: &Fixture, first: &LoopActivation, second: &LoopActivation) {
    assert_eq!(first.mailbox_run_id, second.mailbox_run_id);
    assert_ne!(first.session_id, second.session_id);
    assert!(second.generation > first.generation);
    let before = fixture
        .state
        .agents
        .runtime_history("worker", first.session_id.as_deref().unwrap(), 100, None)
        .await
        .unwrap()
        .unwrap();
    let after = fixture
        .state
        .agents
        .runtime_history("worker", second.session_id.as_deref().unwrap(), 100, None)
        .await
        .unwrap()
        .unwrap();
    assert!(before.closed && after.closed);
    assert_ne!(before.runtime_id, after.runtime_id);
    assert_eq!(
        before.streams[0].native_session_id,
        after.streams[0].native_session_id
    );
    assert!(
        after
            .receipts
            .iter()
            .any(|receipt| receipt.kind == RuntimeRequestKind::ResumeSession
                && receipt.status == RuntimeRequestStatus::Accepted)
    );
    assert!(
        !after
            .receipts
            .iter()
            .any(|receipt| receipt.kind == RuntimeRequestKind::CreateSession)
    );
    let store = LoopStore::new(fixture.state.db.clone());
    assert!(
        store
            .reservation(&fixture.team_id, "worker")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
#[ignore = "requires AGENTHUB_RARA_TEST_BINARY built from PINNED_UPSTREAM_REVISION"]
async fn native_resume_process_retains_history_under_new_activation_sources() {
    let native = NativeFixture::new("clean").await;
    let first = native.fixture.execute("first-conversation").await;
    let second = native.fixture.execute("second-conversation").await;
    assert_new_owner(&native.fixture, &first, &second).await;
    let requests = native.requests.lock().await;
    let work: Vec<_> = requests
        .iter()
        .filter(|request| !semantic_guard::is_guard_request(request))
        .collect();
    assert_eq!(work.len(), 2);
    assert!(
        work[1]["messages"]
            .to_string()
            .contains("Persistent native continuity marker")
    );
    let current_context = work[1]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .rfind(|message| message["role"] == "user")
        .unwrap()["content"]
        .to_string();
    assert!(current_context.contains(&second.id));
    assert!(
        !current_context.contains(&first.id),
        "restored history must not reinstall old activation sources"
    );
    drop(requests);
    native.close().await;
}

#[tokio::test]
#[ignore = "requires AGENTHUB_RARA_TEST_BINARY built from PINNED_UPSTREAM_REVISION"]
async fn native_resume_process_restores_approval_after_kill_without_old_callback_authority() {
    let native = NativeFixture::new("approval").await;
    let (first, old) = tokio::join!(native.fixture.execute("crash-before-answer"), async {
        let pending = native.pending().await;
        native.crash();
        pending
    });
    assert!(!native.fixture.directory.join("native-effects").exists());
    let (second, ()) = tokio::join!(native.fixture.execute("resume-wait"), async {
        let (current, local) = native.pending().await;
        assert_ne!(old.0, current);
        assert_ne!(old.1, local);
        assert_ne!(
            native.answer(&old.0, "once").await,
            crate::acp::AcpPermissionRespondResult::Applied
        );
        assert!(!native.fixture.directory.join("native-effects").exists());
        assert_eq!(
            native.answer(&current, "once").await,
            crate::acp::AcpPermissionRespondResult::Applied
        );
    });
    assert_new_owner(&native.fixture, &first, &second).await;
    assert_eq!(
        std::fs::read_to_string(native.fixture.directory.join("native-effects")).unwrap(),
        "executed\n"
    );
    let history = native
        .fixture
        .state
        .agents
        .runtime_history("worker", second.session_id.as_deref().unwrap(), 100, None)
        .await
        .unwrap()
        .unwrap();
    assert!(
        history
            .receipts
            .iter()
            .any(|receipt| receipt.kind == RuntimeRequestKind::EvaluateReentry)
    );
    assert!(!history.receipts.iter().any(|receipt| matches!(
        receipt.kind,
        RuntimeRequestKind::Prompt | RuntimeRequestKind::GuardedPrompt
    )));
    native.close().await;
}

#[tokio::test]
#[ignore = "requires AGENTHUB_RARA_TEST_BINARY built from PINNED_UPSTREAM_REVISION"]
async fn native_resume_process_reconciles_uncertain_effects_without_replaying_the_answer() {
    let native = NativeFixture::new("uncertain").await;
    let effects = native.fixture.directory.join("native-effects");
    let (first, old) = tokio::join!(
        native.fixture.execute("crash-during-approved-tool"),
        async {
            let (id, local) = native.pending().await;
            native.answer(&id, "once").await;
            tokio::time::timeout(Duration::from_secs(10), async {
                while !effects.exists() {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            native.crash();
            (id, local)
        }
    );
    assert_eq!(std::fs::read_to_string(&effects).unwrap(), "executed\n");
    let previous = native.requests.lock().await.len();
    let mut browser = None;
    let timeout = Duration::from_secs(if browser::directory().is_some() {
        300
    } else {
        15
    });
    let (second, ()) = tokio::join!(
        native
            .fixture
            .execute_with_timeout("resume-uncertain-effect", timeout),
        async {
            let view = native.recovery().await;
            assert_ne!(view.local_session_id, old.1);
            assert_eq!(view.recovery.decisions.len(), 1);
            assert!(matches!(
                view.recovery.decisions[0].state,
                agenthub_rara::DecisionState::Uncertain
            ));
            assert!(view.recovery.waiting_turn_id.is_none());
            let target = agenthub_rara::RecoveryTarget {
                runtime_id: view.runtime_id,
                session_id: view.session_id,
                recovery_id: view.recovery.blocked.unwrap().recovery_id,
            };
            assert_eq!(
                native.requests.lock().await.len(),
                previous + 1,
                "restart admits only the tool-free reentry check"
            );
            assert!(
                native
                    .fixture
                    .state
                    .agents
                    .send_input(
                        "worker",
                        "Do not replay",
                        Some("blocked"),
                        Some(&view.local_session_id)
                    )
                    .await
                    .is_err()
            );
            assert_ne!(
                native.answer(&old.0, "once").await,
                crate::acp::AcpPermissionRespondResult::Applied
            );
            browser =
                browser::Browser::start(&native.fixture, &view.local_session_id, &target).await;
            if let Some(browser) = &browser {
                browser.wait_for_review().await;
            } else {
                native
                    .fixture
                    .state
                    .agents
                    .reconcile_native_recovery(
                        "worker",
                        &view.local_session_id,
                        target,
                        "Old executor retired; one recorded append inspected; do not repeat it"
                            .into(),
                    )
                    .await
                    .unwrap();
            }
            assert_eq!(
                native.requests.lock().await.len(),
                previous + 1,
                "reconciliation must not prompt the model"
            );
            assert_eq!(std::fs::read_to_string(&effects).unwrap(), "executed\n");
        }
    );
    assert_new_owner(&native.fixture, &first, &second).await;
    assert_eq!(second.state, LoopActivationState::Finished);
    let outcome = second.outcome.as_ref().unwrap();
    assert_eq!(outcome.kind.as_str(), "waiting");
    assert_eq!(outcome.wait_reason.unwrap().as_str(), "input");
    assert!(outcome.continuation.is_none());
    assert_eq!(std::fs::read_to_string(&effects).unwrap(), "executed\n");
    let history = native
        .fixture
        .state
        .agents
        .runtime_history("worker", second.session_id.as_deref().unwrap(), 100, None)
        .await
        .unwrap()
        .unwrap();
    assert!(history.receipts.iter().any(|receipt| receipt.kind
        == RuntimeRequestKind::ResolveRecovery
        && receipt.status == RuntimeRequestStatus::Accepted));
    assert!(
        !history
            .receipts
            .iter()
            .any(|receipt| receipt.kind == RuntimeRequestKind::ShellAnswer)
    );
    assert!(!history.receipts.iter().any(|receipt| matches!(
        receipt.kind,
        RuntimeRequestKind::Prompt | RuntimeRequestKind::GuardedPrompt
    )));
    let third = native.fixture.execute("explicit-after-review").await;
    assert_new_owner(&native.fixture, &second, &third).await;
    assert_eq!(std::fs::read_to_string(&effects).unwrap(), "executed\n");
    if let Some(browser) = browser {
        browser.finish().await;
    }
    native.close().await;
}
