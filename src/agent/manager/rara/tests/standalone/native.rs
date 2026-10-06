use std::sync::Arc;

use agent_client_protocol::schema::v1::{RequestPermissionOutcome, SelectedPermissionOutcome};
use agenthub_db::runtime_events::{RuntimeRequestKind, RuntimeRequestStatus};
use axum::{Json, Router, routing::post};
use tokio::sync::Mutex;

use super::*;

const WRAPPER: &str = r#"#!/usr/bin/env python3
import json, os, pathlib, sys
root = pathlib.Path(__file__).parent
settings = json.loads((root / 'native-settings.json').read_text())
os.environ['RARA_HOME'] = str(root / 'native-state')
(root / 'native-pid').write_text(str(os.getpid()))
os.execv(settings['binary'], [settings['binary'], *sys.argv[1:], '--no-extension-discovery', '--no-memory-facilities'])
"#;

struct NativeFixture {
    fixture: Fixture,
    database: PathBuf,
    requests: Arc<Mutex<Vec<Value>>>,
    server: tokio::task::JoinHandle<()>,
}

impl NativeFixture {
    async fn new(mode: &'static str) -> Self {
        let directory =
            std::env::temp_dir().join(format!("standalone-native-db-{}", Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let database = directory.join("control.sqlite");
        let pool = agenthub_db::init_db_at_path(&database).await.unwrap();
        pool.close().await;
        let state = crate::api::team_tests::reopen_test_state_with_db_path(&database).await;
        let mut fixture = Fixture::with_state("normal", state).await;
        let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
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
        std::fs::write(
            fixture.directory.join("native-settings.json"),
            json!({"binary":std::env::var("AGENTHUB_RARA_TEST_BINARY").expect("qualified binary")})
                .to_string(),
        )
        .unwrap();
        std::fs::write(fixture.directory.join("runtime fixture"), WRAPPER).unwrap();
        set_policy(&mut fixture, RaraSessionPolicy::Resume);
        Self {
            fixture,
            database,
            requests,
            server,
        }
    }

    async fn restart_manager(&mut self) {
        let config = (*self.fixture.manager.loop_app_config).clone();
        let events = self.fixture.manager.event_dbs.clone();
        self.fixture.manager.stop_all_on_shutdown().await.unwrap();
        self.fixture
            .manager
            .daemon_tasks
            .shutdown_runtime(Duration::from_secs(5))
            .await
            .unwrap();
        self.fixture.manager.db.close().await;
        let state = crate::api::team_tests::reopen_test_state_with_db_path(&self.database).await;
        let mut manager = (*state.agents).clone().with_loop_app_config(config);
        manager.event_dbs = events;
        manager.mark_exited_on_startup().await.unwrap();
        self.fixture.manager = manager;
    }

    async fn start(&self) -> String {
        self.fixture
            .manager
            .start_agent(&self.fixture.agent_id)
            .await
            .unwrap()
    }

    async fn prompt(&self, local: &str, text: &str) {
        self.fixture
            .manager
            .send_input(&self.fixture.agent_id, text, Some(text), Some(local))
            .await
            .unwrap();
    }

    async fn pending(&self) -> String {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if let Some(id) = sqlx::query_scalar("SELECT id FROM acp_permission_requests WHERE agent_id = ? AND status = 'pending' ORDER BY created_at DESC LIMIT 1")
                    .bind(&self.fixture.agent_id).fetch_optional(&self.fixture.manager.db).await.unwrap() {
                    return id;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.expect("current standalone approval")
    }

    async fn answer(&self, id: &str) -> crate::acp::AcpPermissionRespondResult {
        self.fixture
            .manager
            .permissions
            .respond(
                id,
                RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new("once")),
                Some("once".into()),
                Some("fixture-operator".into()),
            )
            .await
            .unwrap()
    }

    async fn wait_idle(&self) {
        let runtime = self.fixture.runtime().await;
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if runtime.query_recovery().await.is_ok_and(|view| {
                    view.recovery.blocked.is_none() && view.recovery.waiting_turn_id.is_none()
                }) {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("standalone idle")
    }

    async fn crash(&self) {
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
        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(pid as i32),
            nix::sys::signal::Signal::SIGKILL,
        )
        .unwrap();
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if self
                    .fixture
                    .manager
                    .running_session_id_for_agent(&self.fixture.agent_id)
                    .await
                    .is_none()
                {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("verified native crash cleanup");
    }

    async fn assert_resume(&self, first: &str, second: &str) {
        assert_ne!(first, second);
        let before = self
            .fixture
            .manager
            .runtime_history(&self.fixture.agent_id, first, 100, None)
            .await
            .unwrap()
            .unwrap();
        let after = self
            .fixture
            .manager
            .runtime_history(&self.fixture.agent_id, second, 100, None)
            .await
            .unwrap()
            .unwrap();
        assert!(before.closed);
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
    }

    async fn close(self) {
        self.fixture.finish().await;
        self.server.abort();
        std::fs::remove_dir_all(self.database.parent().unwrap()).unwrap();
    }
}

fn response(request: &Value, mode: &str) -> ([(&'static str, &'static str); 1], String) {
    let messages = request["messages"].to_string();
    let completed = request["messages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|message| message["role"] == "tool" && message["tool_call_id"] == "standalone-shell");
    let explicit_new = messages.contains("explicit-after-recovery");
    let (message, finish) = if mode != "clean" && !completed && !explicit_new {
        let command = if mode == "uncertain" {
            "printf 'executed\\n' >> standalone-effects; sleep 60"
        } else {
            "printf 'executed\\n' >> standalone-effects"
        };
        (
            json!({"role":"assistant","content":null,"tool_calls":[{"index":0,"id":"standalone-shell","type":"function","function":{"name":"bash","arguments":json!({"command":command,"sandbox_permissions":"require_escalated","justification":"Verify standalone recovery","prefix_rule":["printf"]}).to_string()}}]}),
            "tool_calls",
        )
    } else {
        (
            json!({"role":"assistant","content":"Standalone continuity marker"}),
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

#[tokio::test]
#[ignore = "requires AGENTHUB_RARA_TEST_BINARY built from PINNED_UPSTREAM_REVISION"]
async fn standalone_native_process_retains_history_across_manager_restart() {
    let mut native = NativeFixture::new("clean").await;
    let first = native.start().await;
    native.prompt(&first, "first-instruction").await;
    native.wait_idle().await;
    native.restart_manager().await;
    let second = native.start().await;
    native.assert_resume(&first, &second).await;
    native.prompt(&second, "second-instruction").await;
    native.wait_idle().await;
    let requests = native.requests.lock().await;
    assert_eq!(requests.len(), 2);
    assert!(
        requests[1]["messages"]
            .to_string()
            .contains("Standalone continuity marker")
    );
    drop(requests);
    native.close().await;
}

#[tokio::test]
#[ignore = "requires AGENTHUB_RARA_TEST_BINARY built from PINNED_UPSTREAM_REVISION"]
async fn standalone_native_process_restores_approval_without_reusing_old_callback() {
    let mut native = NativeFixture::new("approval").await;
    let first = native.start().await;
    native.prompt(&first, "approval-instruction").await;
    let old = native.pending().await;
    native.crash().await;
    native.restart_manager().await;
    let second = native.start().await;
    let current = native.pending().await;
    assert_ne!(old, current);
    assert_ne!(
        native.answer(&old).await,
        crate::acp::AcpPermissionRespondResult::Applied
    );
    assert!(!native.fixture.directory.join("standalone-effects").exists());
    assert_eq!(
        native.answer(&current).await,
        crate::acp::AcpPermissionRespondResult::Applied
    );
    native.wait_idle().await;
    native.assert_resume(&first, &second).await;
    assert_eq!(
        std::fs::read_to_string(native.fixture.directory.join("standalone-effects")).unwrap(),
        "executed\n"
    );
    native.close().await;
}

#[tokio::test]
#[ignore = "requires AGENTHUB_RARA_TEST_BINARY built from PINNED_UPSTREAM_REVISION"]
async fn standalone_native_process_reconciles_without_replaying_uncertain_effects() {
    let mut native = NativeFixture::new("uncertain").await;
    let first = native.start().await;
    native.prompt(&first, "uncertain-instruction").await;
    native.answer(&native.pending().await).await;
    let effects = native.fixture.directory.join("standalone-effects");
    tokio::time::timeout(Duration::from_secs(10), async {
        while !effects.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    native.crash().await;
    let previous = native.requests.lock().await.len();
    native.restart_manager().await;
    let second = native.start().await;
    let runtime = native.fixture.runtime().await;
    let view = runtime.query_recovery().await.unwrap();
    assert!(view.recovery.waiting_turn_id.is_none());
    assert!(matches!(
        view.recovery.decisions[0].state,
        agenthub_rara::DecisionState::Uncertain
    ));
    assert!(
        native
            .fixture
            .manager
            .send_input(
                &native.fixture.agent_id,
                "blocked",
                Some("blocked"),
                Some(&second)
            )
            .await
            .is_err()
    );
    runtime
        .reconcile_recovery(
            agenthub_rara::RecoveryTarget {
                runtime_id: view.runtime_id,
                session_id: view.session_id,
                recovery_id: view.recovery.blocked.unwrap().recovery_id,
            },
            "Observed one append; do not replay it".into(),
        )
        .await
        .unwrap();
    assert_eq!(native.requests.lock().await.len(), previous);
    native.assert_resume(&first, &second).await;
    native.prompt(&second, "explicit-after-recovery").await;
    native.wait_idle().await;
    assert_eq!(std::fs::read_to_string(effects).unwrap(), "executed\n");
    native.close().await;
}
