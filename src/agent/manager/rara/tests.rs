use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use serde_json::Value;

use super::*;
use crate::agent::AgentStatus;
use crate::agent::manager::executor::{AgentExecutor, LocalExecutionRequest, SpawnedLocalProcess};

const PEER: &str = r#"#!/usr/bin/env python3
import json, os, pathlib, signal, subprocess, sys, time
root = pathlib.Path.cwd()
(root / 'pid').write_text(str(os.getpid()))
(root / 'argv.json').write_text(json.dumps(sys.argv[1:]))
mode = (root / 'scenario').read_text()
runtime = 'managed-runtime'
native = 'native-session'
def emit(kind, payload):
    print(json.dumps({'type': kind, 'payload': payload}), flush=True)
def delta(sequence, text):
    emit('event', {'runtime_id': runtime, 'session_id': native, 'event': {
        'event_id': 'event-' + str(sequence), 'sequence': sequence, 'turn_id': 'turn-1',
        'provenance': {'session_id': None},
        'event': {'type': 'assistant', 'payload': {'type': 'text_delta', 'payload': text}}}})
def terminated(*_):
    (root / 'forced').write_text('signal')
    sys.exit(1)
signal.signal(signal.SIGTERM, terminated)
if mode == 'stderr':
    sys.stderr.write('private-diagnostic-token\n' * 20000)
    sys.stderr.flush()
if mode == 'silent':
    time.sleep(30)
if mode == 'malformed':
    print('private-diagnostic-token', flush=True)
    time.sleep(30)
methods = ['session.create', 'session.query_state', 'session.cancel', 'session.interrupt',
    'input.submit_prompt', 'input.submit_follow_up', 'input.answer_user', 'input.answer_plan',
    'input.answer_shell', 'server.shutdown']
if mode == 'missing_method':
    methods.remove('input.answer_shell')
if mode in ('replay', 'gap'):
    methods.append('output.replay')
emit('handshake', {'protocol_version': 1, 'runtime_version': 'fixture', 'runtime_id': runtime,
    'transport': 'stdio-jsonl', 'request_families': ['session', 'input', 'server'] + (['output'] if mode in ('replay', 'gap') else []),
    'request_methods': methods,
    'event_families': ['session', 'input', 'assistant', 'tool', 'approval', 'plan', 'warning', 'error'],
    'capabilities': {'graceful_shutdown': True, 'approval_persistence': False,
        'replay': ({'lifetime': 'runtime', 'max_events_per_session': 256} if mode in ('replay', 'gap') else {'lifetime': 'unavailable'}),
        'request_receipts': {'lifetime': 'runtime', 'max_requests': 32}},
    'provider': None, 'model': None})
if mode == 'descendant':
    child = subprocess.Popen(['sleep', '60'])
    (root / 'descendant').write_text(str(child.pid))
for line in sys.stdin:
    request = json.loads(line)
    if request['type'] == 'replay':
        payload = request['payload']
        assert payload['session_id'] == native
        assert payload['after_sequence'] == 1
        if mode == 'gap':
            emit('ack', {'runtime_id': runtime, 'request_id': payload['request_id'],
                'result': {'status': 'rejected', 'code': 'invalid_request', 'message': 'private-diagnostic-token'}})
            emit('replay_gap', {'runtime_id': runtime, 'request_id': payload['request_id'], 'session_id': native,
                'requested_after': 1, 'oldest_available': 3, 'latest': 3})
        else:
            emit('ack', {'runtime_id': runtime, 'request_id': payload['request_id'],
                'result': {'status': 'accepted', 'session_id': native, 'turn_id': None, 'last_sequence': 3}})
            delta(2, 'second')
            delta(3, 'third')
        continue
    if request['type'] == 'control':
        envelope = request['payload']['envelope']
        assert envelope['request']['payload']['type'] == 'create_session'
        assert envelope['provenance']['session_id'] is None
        if mode == 'create_drop':
            sys.exit(0)
        if mode == 'create_reject':
            emit('ack', {'runtime_id': runtime, 'request_id': envelope['request_id'],
                'result': {'status': 'rejected', 'code': 'busy', 'message': 'private-diagnostic-token'}})
            continue
        emit('ack', {'runtime_id': runtime, 'request_id': envelope['request_id'],
            'result': {'status': 'accepted', 'session_id': native, 'turn_id': None, 'last_sequence': 1}})
        emit('event', {'runtime_id': runtime, 'session_id': native, 'event': {
            'event_id': 'event-1', 'sequence': 1, 'provenance': {'session_id': None},
            'event': {'type': 'session', 'payload': {'type': 'created', 'payload': {'session_id': native}}}}})
        if mode == 'exit_zero':
            time.sleep(0.05)
            sys.exit(0)
        if mode in ('replay', 'gap'):
            delta(3, 'third')
        continue
    assert request['type'] == 'shutdown'
    assert request['payload']['runtime_id'] == runtime
    request_id = request['payload']['request_id']
    (root / 'shutdown').write_text(request_id)
    emit('ack', {'runtime_id': runtime, 'request_id': request_id,
        'result': {'status': 'accepted', 'session_id': None, 'turn_id': None, 'last_sequence': None}})
    if mode in ('stall', 'descendant'):
        time.sleep(30)
    (root / 'complete').write_text(request_id)
    emit('shutdown_complete', {'runtime_id': runtime, 'request_id': request_id})
    sys.exit(0)
"#;

struct Fixture {
    manager: AgentManager,
    agent_id: String,
    directory: PathBuf,
}

impl Fixture {
    async fn new(scenario: &str) -> Self {
        let state = crate::api::team_tests::build_test_state().await;
        let directory =
            std::env::temp_dir().join(format!("direct-runtime-test-{}", Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("scenario"), scenario).unwrap();
        let script = directory.join("runtime fixture");
        std::fs::write(&script, PEER).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        let manager = (*state.agents)
            .clone()
            .with_loop_app_config(agenthub_config::AppConfig {
                rara: Some(agenthub_config::RaraConfig {
                    binary: Some(script.to_string_lossy().into_owned()),
                    startup_timeout_seconds: Some(1),
                    shutdown_timeout_seconds: Some(1),
                    ..Default::default()
                }),
                ..Default::default()
            });
        let agent_id = format!("direct-{}", Uuid::new_v4());
        sqlx::query("INSERT INTO agents (id, name, workdir, command, args, worktree_mode, status, created_at, updated_at) VALUES (?, ?, ?, 'rara', '[]', 'use_existing', 'stopped', 1, 1)")
            .bind(&agent_id).bind(&agent_id).bind(directory.to_str().unwrap())
            .execute(&manager.db).await.unwrap();
        Self {
            manager,
            agent_id,
            directory,
        }
    }

    async fn assert_clean(&self) {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if !self.manager.inner.read().await.contains_key(&self.agent_id) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert!(
            !self
                .manager
                .process_supervisor
                .has_actor_process(&self.agent_id)
                .await
        );
        assert!(!self.manager.inner.read().await.contains_key(&self.agent_id));
        let running: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM agent_sessions WHERE agent_id = ? AND ended_at IS NULL",
        )
        .bind(&self.agent_id)
        .fetch_one(&self.manager.db)
        .await
        .unwrap();
        assert_eq!(running, 0);
    }

    async fn finish(self) {
        self.manager.stop_all_on_shutdown().await.unwrap();
        self.manager
            .daemon_tasks
            .shutdown_runtime(Duration::from_secs(5))
            .await
            .unwrap();
        std::fs::remove_dir_all(&self.directory).unwrap();
    }
}

#[tokio::test]
async fn managed_start_drains_stderr_and_preserves_protocol_ownership() {
    let fixture = Fixture::new("stderr").await;
    let session = fixture
        .manager
        .start_agent(&fixture.agent_id)
        .await
        .unwrap();
    {
        let handles = fixture.manager.inner.read().await;
        let AgentInput::Rara(client) = &handles[&fixture.agent_id].input else {
            panic!("direct handle");
        };
        assert_eq!(client.handshake().runtime_id, "managed-runtime");
        assert_ne!(client.handshake().runtime_id, session);
    }
    let args: Value =
        serde_json::from_slice(&std::fs::read(fixture.directory.join("argv.json")).unwrap())
            .unwrap();
    assert_eq!(
        &args.as_array().unwrap()[..5],
        &[
            "app-server",
            "--protocol-version",
            "1",
            "--transport",
            "stdio-jsonl"
        ]
        .map(Value::from)
    );
    assert_eq!(args[6], fixture.directory.to_str().unwrap());
    let error = fixture
        .manager
        .send_input(
            &fixture.agent_id,
            "must not enter stdin",
            None,
            Some(&session),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("input mapping is unavailable"));
    let (first, second) = tokio::join!(
        fixture.manager.stop_agent(&fixture.agent_id),
        fixture.manager.stop_agent(&fixture.agent_id),
    );
    first.unwrap();
    second.unwrap();
    assert!(fixture.directory.join("complete").exists());
    assert!(!fixture.directory.join("forced").exists());
    fixture.assert_clean().await;
    let events = fixture
        .manager
        .list_events(&fixture.agent_id, 100, None)
        .await
        .unwrap();
    assert!(
        events
            .iter()
            .all(|event| !event.message.contains("private-diagnostic-token"))
    );
    fixture.finish().await;
}

#[tokio::test]
async fn managed_handshake_failures_clean_the_owned_process() {
    for scenario in ["silent", "malformed", "missing_method"] {
        let fixture = Fixture::new(scenario).await;
        let error = fixture
            .manager
            .start_agent(&fixture.agent_id)
            .await
            .unwrap_err();
        assert!(!error.to_string().contains("private-diagnostic-token"));
        assert_eq!(
            fixture
                .manager
                .get_agent(&fixture.agent_id)
                .await
                .unwrap()
                .status,
            AgentStatus::Failed
        );
        fixture.assert_clean().await;
        fixture.finish().await;
    }
}

#[tokio::test]
async fn managed_shutdown_timeout_uses_supervisor_fallback() {
    for scenario in ["stall", "descendant"] {
        let fixture = Fixture::new(scenario).await;
        fixture
            .manager
            .start_agent(&fixture.agent_id)
            .await
            .unwrap();
        fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
        assert!(fixture.directory.join("shutdown").exists());
        assert!(!fixture.directory.join("complete").exists());
        assert!(fixture.directory.join("forced").exists());
        fixture.assert_clean().await;
        fixture.finish().await;
    }
}

#[tokio::test]
async fn daemon_shutdown_uses_semantic_completion_before_cleanup() {
    let fixture = Fixture::new("normal").await;
    fixture
        .manager
        .start_agent(&fixture.agent_id)
        .await
        .unwrap();
    fixture.manager.stop_all_on_shutdown().await.unwrap();
    assert!(fixture.directory.join("complete").exists());
    assert!(!fixture.directory.join("forced").exists());
    fixture.assert_clean().await;
    assert!(
        fixture
            .manager
            .start_agent(&fixture.agent_id)
            .await
            .is_err()
    );
    fixture.finish().await;
}

#[tokio::test]
async fn clean_exit_without_semantic_shutdown_is_a_transport_failure() {
    let fixture = Fixture::new("exit_zero").await;
    fixture
        .manager
        .start_agent(&fixture.agent_id)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if fixture
                .manager
                .get_agent(&fixture.agent_id)
                .await
                .unwrap()
                .status
                == AgentStatus::Failed
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    fixture.assert_clean().await;
    fixture.finish().await;
}

#[tokio::test]
async fn unsupported_placement_and_arguments_fail_before_spawn() {
    let fixture = Fixture::new("normal").await;
    let mut agent = fixture.manager.get_agent(&fixture.agent_id).await.unwrap();
    agent.target_node_id = Some("remote".into());
    assert!(
        fixture
            .manager
            .rara_launch_configuration(&agent, None)
            .is_err()
    );
    agent.target_node_id = None;
    agent.args = vec!["--dangerously-skip-permissions".into()];
    assert!(
        fixture
            .manager
            .rara_launch_configuration(&agent, None)
            .is_err()
    );
    agent.args.clear();
    agent.agent_loop_enabled = true;
    assert!(
        fixture
            .manager
            .rara_launch_configuration(&agent, None)
            .is_err()
    );
    agent.agent_loop_enabled = false;
    agent.thinking_level = Some("high".into());
    assert!(
        fixture
            .manager
            .rara_launch_configuration(&agent, None)
            .is_err()
    );
    agent.command = "cat".into();
    assert!(
        fixture
            .manager
            .rara_launch_configuration(&agent, None)
            .unwrap()
            .is_none()
    );
    assert!(!fixture.directory.join("pid").exists());
    fixture.finish().await;
}

struct NativeFixtureEnvironment {
    delegate: std::sync::Arc<dyn AgentExecutor>,
    state: PathBuf,
}

#[tokio::test]
async fn managed_creation_records_rejection_and_unknown_outcomes_without_retry() {
    for (scenario, expected) in [
        ("create_reject", "rejected"),
        ("create_drop", "outcome_unknown"),
    ] {
        let fixture = Fixture::new(scenario).await;
        assert!(
            fixture
                .manager
                .start_agent(&fixture.agent_id)
                .await
                .is_err()
        );
        let pool = fixture
            .manager
            .event_dbs
            .pool_for_agent(&fixture.agent_id)
            .await
            .unwrap();
        let rows: Vec<(String, Option<String>)> =
            sqlx::query_as("SELECT status, ack_json FROM runtime_control_receipts")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, expected);
        assert!(
            !rows[0]
                .1
                .as_deref()
                .unwrap_or("")
                .contains("private-diagnostic-token")
        );
        let closed: bool = sqlx::query_scalar("SELECT closed FROM runtime_event_owners")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(closed);
        fixture.assert_clean().await;
        fixture.finish().await;
    }
}

#[tokio::test]
async fn managed_replay_repairs_order_and_persists_before_history_delivery() {
    let fixture = Fixture::new("replay").await;
    let local = fixture
        .manager
        .start_agent(&fixture.agent_id)
        .await
        .unwrap();
    let pool = fixture
        .manager
        .event_dbs
        .pool_for_agent(&fixture.agent_id)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let sequence: i64 =
                sqlx::query_scalar("SELECT last_sequence FROM runtime_event_streams")
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            if sequence == 3 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let rows: Vec<(String, Vec<u8>)> = sqlx::query_as("SELECT e.session_id, e.message FROM agent_events e JOIN runtime_event_history h ON h.history_id = e.id ORDER BY e.id")
        .fetch_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 2);
    for ((session, bytes), (text, index)) in rows.iter().zip([("second", 0), ("third", 1)]) {
        assert_eq!(session, &local);
        let value: Value = serde_json::from_str(
            &crate::agent::event_message_codec::decode_message_from_storage(bytes),
        )
        .unwrap();
        assert_eq!(value["text"], text);
        assert_eq!(value["chunk_index"], index);
    }
    fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM runtime_control_receipts WHERE status = 'accepted'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 2);
    fixture.assert_clean().await;
    fixture.finish().await;
}

#[tokio::test]
async fn managed_replay_gap_preserves_cursor_and_fails_visible() {
    let fixture = Fixture::new("gap").await;
    fixture
        .manager
        .start_agent(&fixture.agent_id)
        .await
        .unwrap();
    let pool = fixture
        .manager
        .event_dbs
        .pool_for_agent(&fixture.agent_id)
        .await
        .unwrap();
    fixture.assert_clean().await;
    let cursor: (i64, Option<i64>, Option<i64>) =
        sqlx::query_as("SELECT last_sequence, gap_after, gap_oldest FROM runtime_event_streams")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(cursor, (1, Some(1), Some(3)));
    assert_eq!(
        fixture
            .manager
            .get_agent(&fixture.agent_id)
            .await
            .unwrap()
            .status,
        AgentStatus::Failed
    );
    fixture.finish().await;
}

#[async_trait::async_trait]
impl AgentExecutor for NativeFixtureEnvironment {
    async fn spawn_process(
        &self,
        mut request: LocalExecutionRequest,
    ) -> anyhow::Result<SpawnedLocalProcess> {
        request
            .extra_env
            .push(("RARA_HOME".into(), self.state.to_str().unwrap().into()));
        request.args.extend([
            "--no-extension-discovery".into(),
            "--no-memory-facilities".into(),
        ]);
        self.delegate.spawn_process(request).await
    }
}

#[tokio::test]
#[ignore = "requires AGENTHUB_RARA_TEST_BINARY built from PINNED_UPSTREAM_REVISION"]
async fn managed_native_process_transport() {
    let binary = std::env::var("AGENTHUB_RARA_TEST_BINARY").expect("pinned binary path");
    let mut fixture = Fixture::new("normal").await;
    let native_state = fixture.directory.join("state");
    std::fs::create_dir(&native_state).unwrap();
    std::fs::write(
        native_state.join("config.json"),
        serde_json::to_vec(&serde_json::json!({
            "provider":"deepseek","api_key":"fixture-key","model":"fixture-model",
            "base_url":"http://127.0.0.1:9/v1"
        }))
        .unwrap(),
    )
    .unwrap();
    fixture.manager.local_executor = std::sync::Arc::new(NativeFixtureEnvironment {
        delegate: fixture.manager.local_executor.clone(),
        state: native_state,
    });
    fixture.manager = fixture
        .manager
        .with_loop_app_config(agenthub_config::AppConfig {
            rara: Some(agenthub_config::RaraConfig {
                binary: Some(binary),
                ..Default::default()
            }),
            ..Default::default()
        });
    let launch_id = fixture
        .manager
        .start_agent(&fixture.agent_id)
        .await
        .unwrap();
    {
        let handles = fixture.manager.inner.read().await;
        let AgentInput::Rara(client) = &handles[&fixture.agent_id].input else {
            panic!("direct handle");
        };
        assert_ne!(client.handshake().runtime_id, launch_id);
        assert_eq!(
            client.handshake().protocol_version,
            agenthub_rara::PROTOCOL_VERSION
        );
    }
    fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
    fixture.assert_clean().await;
    assert_eq!(
        fixture
            .manager
            .get_agent(&fixture.agent_id)
            .await
            .unwrap()
            .status,
        AgentStatus::Stopped
    );
    fixture.finish().await;
}
