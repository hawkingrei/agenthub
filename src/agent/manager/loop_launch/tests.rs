use std::os::unix::fs::PermissionsExt;

use agenthub_agent_domain::loop_runtime::{
    LoopActivationState, LoopAdmission, LoopLimits, LoopPolicyState, LoopSourceReferences,
    LoopTriggerInput, LoopTriggerKind,
};
use agenthub_db::loop_runtime::LoopPolicyUpdate;

use super::*;

mod mcp;

const PROVIDER: &str = r#"#!/usr/bin/env python3
import json, os, subprocess, sys, uuid
log_path, mode, control_binary = sys.argv[1:]
shim = None
def mcp_call(message):
    shim.stdin.write(json.dumps(message) + '\n')
    shim.stdin.flush()
    return json.loads(shim.stdout.readline())
for line in sys.stdin:
    request = json.loads(line)
    with open(log_path, 'a') as log:
        log.write(json.dumps({'method': request.get('method'), 'legacy_token_inherited': 'AGENTHUB_INTERNAL_GRPC_TOKEN' in os.environ}) + '\n')
    if 'id' not in request:
        continue
    method = request['method']
    if method == 'initialize':
        result = {'protocolVersion': 1, 'agentCapabilities': {'loadSession': True}}
    elif method == 'session/new':
        if mode == 'mcp':
            private_keys = ['TEST_MEM_UPSTREAM_KEY', 'TEST_OTHER_MEM_KEY', 'NMEM_API_KEY', 'NMEM_API_URL', 'NOWLEDGE_MEM_HEADERS', 'MCP_HTTP_HEADERS']
            assert all(key not in os.environ for key in private_keys)
            servers = request['params']['mcpServers']
            assert len(servers) == 1
            server = servers[0]
            assert server['args'] == ['mcp-proxy', '--server-id', 'nowledge-mem']
            assert 'url' not in server and 'headers' not in server
            env = dict(os.environ)
            env.update({item['name']:item['value'] for item in server['env']})
            shim = subprocess.Popen([server['command']] + server['args'], env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            with open('/proc/' + str(shim.pid) + '/environ', 'rb') as environ:
                inherited = environ.read().split(b'\0')
            assert all(not any(item.startswith(key.encode() + b'=') for item in inherited) for key in private_keys)
            initialized = mcp_call({'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-11-25','capabilities':{},'clientInfo':{'name':'fake-acp','version':'1'}}})
            assert initialized['result']['protocolVersion'] == '2025-11-25'
            assert 'resources' in initialized['result']['capabilities'] and 'prompts' in initialized['result']['capabilities']
            shim.stdin.write(json.dumps({'jsonrpc':'2.0','method':'notifications/initialized'}) + '\n')
            tools = mcp_call({'jsonrpc':'2.0','id':2,'method':'tools/list'})
            assert tools['result']['tools'][0]['name'] == 'fixture_write'
            with open(log_path, 'a') as log:
                log.write(json.dumps({'mcp_bootstrap':True, 'server':server}) + '\n')
        result = {'sessionId': str(uuid.uuid4())}
    elif method == 'session/prompt':
        if mode == 'mcp':
            result = mcp_call({'jsonrpc':'2.0','id':3,'method':'tools/call','params':{'name':'fixture_write','arguments':{'body':'private-business-body'}}})
            assert result['result']['structuredContent']['written'] is True
            for request_id, memory, allowed in [(4, 'allowed', True), (5, 'foreign', False)]:
                result = mcp_call({'jsonrpc':'2.0','id':request_id,'method':'tools/call','params':{'name':'fixture_lookup','arguments':{'memory_id':memory}}})
                assert (result['result'].get('isError') is not True) == allowed
                assert result['result']['extension'] == 'preserved'
            for request_id, uri, allowed in [(6, 'mem://allowed', True), (7, 'mem://foreign', False)]:
                result = mcp_call({'jsonrpc':'2.0','id':request_id,'method':'resources/read','params':{'uri':uri}})
                if allowed:
                    assert result['result']['contents'][0]['uri'] == uri
                else:
                    assert result['error']['code'] == -32002 and result['error']['data'] == {'native':True}
            prompt = mcp_call({'jsonrpc':'2.0','id':8,'method':'prompts/get','params':{'name':'brief'}})
            assert prompt['result']['extension'] == 'preserved'
            shim.stdin.close()
            assert shim.wait(timeout=5) == 0
            with open(log_path, 'a') as log:
                log.write(json.dumps({'mcp_write':True}) + '\n')
        if mode in ['finish', 'mcp']:
            for command in ['team-members', 'team-tasks', 'inbox']:
                recovery = subprocess.run([control_binary, 'actor', command, '--json'], capture_output=True, text=True)
                if recovery.returncode:
                    with open(log_path, 'a') as log:
                        log.write(json.dumps({'recovery_error': recovery.stderr}) + '\n')
                    print(recovery.stderr, file=sys.stderr, flush=True)
                    sys.exit(2)
                with open(log_path, 'a') as log:
                    log.write(json.dumps({'recovered':command}) + '\n')
            path = os.path.join(os.getcwd(), 'loop-outcome.json')
            with open(path, 'w') as outcome:
                json.dump({'kind':'no_actionable_work'}, outcome)
            subprocess.run([control_binary, 'actor', 'loop-finish', '--outcome-file', path, '--json'], stdout=subprocess.DEVNULL)
        result = {'stopReason':'end_turn'}
    else:
        result = {}
    print(json.dumps({'jsonrpc':'2.0', 'id':request['id'], 'result':result}), flush=True)
"#;

struct Fixture {
    state: crate::state::AppState,
    directory: std::path::PathBuf,
    team_id: String,
}

impl Fixture {
    async fn new(mode: &str) -> Self {
        Self::new_with_mem(mode, None).await
    }

    async fn new_with_mem(mode: &str, endpoint: Option<&str>) -> Self {
        let mut state = crate::api::team_tests::build_test_state().await;
        sqlx::query("ALTER TABLE agents ADD COLUMN runtime_model TEXT")
            .execute(&state.db)
            .await
            .unwrap();
        let directory =
            std::env::temp_dir().join(format!("agenthub-loop-provider-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let program = directory.join("claude-agent-acp");
        std::fs::write(&program, PROVIDER).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        let control = crate::agenthub_binary::resolve_agenthub_binary_path().unwrap();
        for actor in ["planner", "worker"] {
            sqlx::query("INSERT INTO agents(id, name, workdir, command, args, worktree_mode, status, runtime_model, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 'use_existing', 'stopped', 'model-one', 1, 1) ON CONFLICT(id) DO UPDATE SET workdir = excluded.workdir, command = excluded.command, args = excluded.args, worktree_mode = excluded.worktree_mode, status = excluded.status, runtime_model = excluded.runtime_model")
                .bind(actor).bind(actor).bind(directory.to_string_lossy().as_ref()).bind(program.to_string_lossy().as_ref())
                .bind(serde_json::json!([directory.join("requests.jsonl"), mode, control]).to_string()).execute(&state.db).await.unwrap();
        }
        let team = state.teams.create_team(crate::team::TeamDefinitionConfig {
            name: format!("loop-provider-{}", uuid::Uuid::new_v4()), description: None,
            spec: serde_json::json!({"execution_mode":"loop", "entrypoint":"planner", "members":[{"member_id":"planner","role":"coordinator"},{"member_id":"worker","role":"worker"}]}),
        }).await.unwrap();
        let mut config = agenthub_config::AppConfig {
            internal_grpc: Some(agenthub_config::InternalGrpcConfig {
                enabled: Some(true),
                listen: Some("127.0.0.1:0".into()),
                security: Some(agenthub_config::InternalGrpcSecurityConfig {
                    mode: Some("disabled".into()),
                    cert_dir: Some(directory.join("certs").to_string_lossy().to_string()),
                }),
                auth: None,
                bootstrap: None,
            }),
            ..Default::default()
        };
        if let Some(endpoint) = endpoint {
            use agenthub_config::{
                NowledgeMemConfig, NowledgeMemProfileConfig, NowledgeMemTeamBindingConfig,
            };
            use std::collections::HashMap;
            config.nowledge_mem = Some(NowledgeMemConfig {
                profiles: Some(HashMap::from([
                    (
                        "team-profile".into(),
                        NowledgeMemProfileConfig {
                            endpoint: endpoint.into(),
                            credential_env: "TEST_MEM_UPSTREAM_KEY".into(),
                            tool_set: Some("external-agent".into()),
                        },
                    ),
                    (
                        "unused-profile".into(),
                        NowledgeMemProfileConfig {
                            endpoint: endpoint.into(),
                            credential_env: "TEST_OTHER_MEM_KEY".into(),
                            tool_set: None,
                        },
                    ),
                ])),
                team_bindings: Some(HashMap::from([(
                    team.id.clone(),
                    NowledgeMemTeamBindingConfig {
                        profile: "team-profile".into(),
                        space_id: "space-a".into(),
                        actor_profiles: None,
                    },
                )])),
            });
            state.agents = Arc::new((*state.agents).clone().with_loop_app_config(config.clone()));
            agenthub_db::mcp_operations::migrate_mcp_operations(&state.db)
                .await
                .unwrap();
            let daemon = agenthub_db::claim_daemon_generation(
                &state.db,
                "main",
                "mcp-fixture",
                1,
                Utc::now().timestamp(),
            )
            .await
            .unwrap();
            state
                .agents
                .initialize_mcp_proxy(
                    agenthub_db::mcp_operations::McpOperationStore::new(state.db.clone(), daemon),
                    Vec::new(),
                )
                .unwrap();
        }
        crate::internal::maybe_spawn_internal_grpc(state.clone(), &config)
            .await
            .unwrap();
        LoopStore::new(state.db.clone())
            .configure(
                LoopPolicyUpdate {
                    actor_id: "worker",
                    team_id: &team.id,
                    expected_revision: 1,
                    state: LoopPolicyState::Enabled,
                    session_policy: LoopSessionPolicy::Fresh,
                    limits: &LoopLimits::default(),
                },
                Utc::now().timestamp(),
            )
            .await
            .unwrap();
        Self {
            state,
            directory,
            team_id: team.id,
        }
    }

    async fn admit(&self, key: &str) -> LoopReservation {
        let store = LoopStore::new(self.state.db.clone());
        let now = Utc::now().timestamp();
        let trigger = store
            .accept_trigger(
                &LoopTriggerInput {
                    actor_id: "worker".into(),
                    team_id: self.team_id.clone(),
                    kind: LoopTriggerKind::Operator,
                    source_key: key.into(),
                    due_at: None,
                    references: LoopSourceReferences::default(),
                },
                now,
            )
            .await
            .unwrap();
        let LoopAdmission::Admitted(reservation) = store
            .admit(
                &self.team_id,
                &trigger.activation_id,
                self.state.agents.loop_owner_id(),
                now,
            )
            .await
            .unwrap()
        else {
            panic!("not admitted");
        };
        self.state
            .agents
            .track_loop_reservation(reservation.clone())
            .await
            .unwrap();
        reservation
    }

    async fn execute(&self, key: &str) -> agenthub_agent_domain::loop_runtime::LoopActivation {
        let reservation = self.admit(key).await;
        let activation_id = reservation.activation_id.clone().unwrap();
        let store = LoopStore::new(self.state.db.clone());
        tokio::time::timeout(
            Duration::from_secs(15),
            self.state
                .agents
                .execute_loop_activation(self.state.teams.clone(), reservation),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(
            store
                .reservation(&self.team_id, "worker")
                .await
                .unwrap()
                .is_none()
        );
        store
            .activation(&self.team_id, &activation_id)
            .await
            .unwrap()
            .unwrap()
    }

    async fn close(self) {
        self.state.agents.stop_all_on_shutdown().await.unwrap();
        self.state
            .agents
            .daemon_tasks()
            .shutdown_runtime(Duration::from_secs(5))
            .await
            .unwrap();
        self.state
            .agents
            .daemon_tasks()
            .shutdown_background(Duration::from_secs(5))
            .await
            .unwrap();
        std::fs::remove_dir_all(self.directory).unwrap();
    }
}

#[tokio::test]
async fn loop_provider_fresh_recovery_preserves_mailbox_and_records_missing_outcome() {
    let fixture = Fixture::new("no-outcome").await;
    let first = fixture.execute("first").await;
    assert_eq!(first.state, LoopActivationState::Interrupted);
    assert!(first.outcome.is_none());
    assert_eq!(
        first.launch.as_ref().unwrap().model.as_deref(),
        Some("model-one")
    );
    sqlx::query("UPDATE agents SET runtime_model = 'model-two' WHERE id = 'worker'")
        .execute(&fixture.state.db)
        .await
        .unwrap();
    let second = fixture.execute("second").await;
    assert_eq!(second.mailbox_run_id, first.mailbox_run_id);
    assert_ne!(second.session_id, first.session_id);
    assert_eq!(second.generation, first.generation + 1);
    assert_eq!(
        second.launch.as_ref().unwrap().model.as_deref(),
        Some("model-two")
    );
    assert_ne!(
        second.launch.as_ref().unwrap().configuration_digest,
        first.launch.as_ref().unwrap().configuration_digest
    );
    let requests = std::fs::read_to_string(fixture.directory.join("requests.jsonl")).unwrap();
    assert_eq!(requests.matches("session/new").count(), 2);
    assert_eq!(requests.matches("session/prompt").count(), 2);
    assert!(!requests.contains("session/load"));
    assert!(!requests.contains("true"));
    let steps: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM team_steps")
        .fetch_one(&fixture.state.db)
        .await
        .unwrap();
    assert_eq!(steps, 0);
    fixture.close().await;
}

#[tokio::test]
async fn loop_provider_recovers_canonical_context_and_finishes_through_signed_cli() {
    let fixture = Fixture::new("finish").await;
    let activation = fixture.execute("finish").await;
    let requests = std::fs::read_to_string(fixture.directory.join("requests.jsonl")).unwrap();
    assert_eq!(
        activation.state,
        LoopActivationState::Finished,
        "{requests}"
    );
    assert!(activation.outcome.is_some());
    for command in ["team-members", "team-tasks", "inbox"] {
        assert!(
            requests.contains(command),
            "missing recovery command: {command}"
        );
    }
    assert!(
        fixture
            .state
            .agents
            .loop_credentials
            .lock()
            .await
            .is_empty()
    );
    fixture.close().await;
}
