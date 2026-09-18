use std::os::unix::fs::PermissionsExt;

use agenthub_agent_domain::loop_runtime::{
    LoopActivationState, LoopAdmission, LoopLimits, LoopPolicyState, LoopSourceReferences,
    LoopTriggerInput, LoopTriggerKind,
};
use agenthub_db::loop_runtime::LoopPolicyUpdate;

use super::*;

mod apps;
mod browser;
mod mcp;
mod mem;
mod roles;

const PROVIDER: &str = r#"#!/usr/bin/env python3
import json, os, subprocess, sys, time, uuid
log_path, mode, control_binary = sys.argv[1:]
if mode == 'mem':
    import mem_provider
if mode == 'apps':
    import app_provider
shim = None
def mcp_call(message):
    shim.stdin.write(json.dumps(message) + '\n')
    shim.stdin.flush()
    return json.loads(shim.stdout.readline())
def actor(*args):
    result = subprocess.run([control_binary, 'actor', *args, '--json'], capture_output=True, text=True)
    if result.returncode:
        with open(log_path, 'a') as log:
            log.write(json.dumps({'cli_error': result.stderr, 'command': args[0]}) + '\n')
        raise RuntimeError(result.stderr)
    return json.loads(result.stdout)
for line in sys.stdin:
    request = json.loads(line)
    with open(log_path, 'a') as log:
        log.write(json.dumps({'method': request.get('method'), 'legacy_token_inherited': 'AGENTHUB_INTERNAL_GRPC_TOKEN' in os.environ}) + '\n')
    if 'id' not in request:
        continue
    method = request['method']
    if method == 'initialize':
        result = {'protocolVersion': 1, 'agentCapabilities': {'loadSession': True}}
    elif method in ['session/new', 'session/load']:
        if mode == 'role-pin':
            with open(os.path.join(os.getcwd(), 'role-ready'), 'w') as ready:
                ready.write(os.environ['AGENTHUB_LOOP_ACTIVATION_ID'])
            while not os.path.exists(os.path.join(os.getcwd(), 'role-release')):
                time.sleep(0.01)
        if mode == 'mem':
            mem_provider.start(request['params'], log_path)
        if mode == 'apps':
            app_provider.start(request['params'], log_path)
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
        result = {'sessionId': str(uuid.uuid4())} if method == 'session/new' else {}
    elif method == 'session/prompt':
        # The opt-in browser fixture can hold execution across page closure.
        while os.path.exists(os.path.join(os.getcwd(), 'browser-hold')):
            time.sleep(0.05)
        if mode == 'role-pin':
            with open(log_path, 'a') as log:
                log.write(json.dumps({'role_prompt': request['params']['prompt']}) + '\n')
        if mode == 'mem':
            mem_provider.work(request['params'], actor, log_path)
        if mode == 'apps':
            app_provider.work(actor, log_path)
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
        if mode == 'handoff':
            page = actor('loop-context', '--limit', '1')
            activation = page['activation']
            prompt = '\n'.join(block.get('text', '') for block in request['params']['prompt'])
            role_label = 'Team Coordinator' if activation['actor_id'] == 'planner' else 'Team Worker'
            assert prompt.count('You are ' + ('the ' if activation['actor_id'] == 'planner' else 'a ') + role_label) == 1
            assert '<name>team-loop-runtime</name>' in prompt
            assert 'team-worker-executor' not in prompt and 'Team workflow phases' not in prompt
            for round_number in range(2):
                print(json.dumps({'jsonrpc':'2.0', 'method':'session/update', 'params':{'sessionId':request['params']['sessionId'], 'update':{'sessionUpdate':'agent_message_chunk', 'content':{'type':'text', 'text':'Recover durable work round ' + str(round_number)}}}}), flush=True)
            sources = page['sources']
            while page['next_cursor'] is not None:
                page = actor('loop-context', '--limit', '1', '--after-source-id', page['next_cursor'])
                sources.extend(page['sources'])
            details = [actor('loop-source', '--source-id', source['id']) for source in sources]
            with open(log_path, 'a') as log:
                log.write(json.dumps({'actor': activation['actor_id'], 'sources': details}) + '\n')
            if activation['actor_id'] == 'planner' and not any(d['mailbox_message'] for d in details):
                actor('team-task-create', '--title', 'Offline review', '--priority', 'medium', '--assigned-member-id', 'worker')
                actor('send', '--to', 'worker', '--text', 'dispatch evidence', '--idempotency-key', 'dispatch:one')
            elif activation['actor_id'] == 'worker':
                assert any(d['mailbox_message'] and d['mailbox_message']['payload']['text'] == 'dispatch evidence' for d in details)
                task_ids = {d['source']['input']['references']['task_id'] for d in details if d['source']['input']['references']['task_id']}
                assert len(task_ids) == 1
                task_id = next(iter(task_ids))
                detail = actor('team-task-show', '--task-id', task_id)
                assert detail['task']['assigned_member_id'] == 'worker'
                denied = subprocess.run([control_binary, 'actor', 'team-task-update', '--team-id', activation['team_id'], '--task-id', task_id, '--status', 'completed', '--note-kind', 'result', '--note', 'Self acceptance is forbidden', '--json'], capture_output=True, text=True)
                assert denied.returncode != 0 and 'coordinator' in denied.stderr.lower(), denied.stderr
                assert actor('team-task-show', '--task-id', task_id)['task']['status'] == 'open'
                with open(log_path, 'a') as log:
                    log.write(json.dumps({'worker_acceptance_denied': True}) + '\n')
                actor('team-task-note', '--task-id', task_id, '--kind', 'result', '--text', 'Review evidence is ready')
                actor('send', '--to', 'planner', '--text', 'worker report', '--idempotency-key', 'report:one')
            else:
                assert any(d['mailbox_message'] and d['mailbox_message']['payload']['text'] == 'worker report' for d in details)
                tasks = actor('team-tasks')
                task = next(task for task in tasks if task['title'] == 'Offline review')
                detail = actor('team-task-show', '--task-id', task['id'])
                assert any(note['from_actor_id'] == 'worker' and note['text'] == 'Review evidence is ready' for note in detail['notes'])
                actor('team-task-update', '--team-id', activation['team_id'], '--task-id', task['id'], '--status', 'completed', '--note-kind', 'decision', '--note', 'Accepted after reviewing worker evidence')
                with open(log_path, 'a') as log:
                    log.write(json.dumps({'coordinator_accepted': task['id']}) + '\n')
            path = os.path.join(os.getcwd(), 'loop-outcome.json')
            with open(path, 'w') as outcome:
                json.dump({'kind':'handoff'}, outcome)
            actor('loop-finish', '--outcome-file', path)
        if mode == 'scheduling':
            activation = actor('loop-context')['activation']
            target = 'planner' if activation['actor_id'] == 'worker' else 'worker'
            path = os.path.join(os.getcwd(), 'loop-schedule.json')
            intent = {'source_key': 'cycle:' + activation['id'], 'schedule': {'kind': 'due', 'due_at': int(time.time())}}
            with open(path, 'w') as output:
                json.dump(intent, output)
            receipt = actor('loop-schedule', '--member-id', target, '--request-file', path)
            registration_id = receipt['registration']['id']
            assert actor('loop-schedule-show', '--registration-id', registration_id)['registration']['input']['actor_id'] == target
            page = actor('loop-schedules', '--member-id', target, '--limit', '1')
            registered_ids = [r['id'] for r in page['registrations']]
            while page['next_cursor'] is not None:
                page = actor('loop-schedules', '--member-id', target, '--limit', '1', '--after-registration-id', page['next_cursor'])
                registered_ids.extend(r['id'] for r in page['registrations'])
            assert registration_id in registered_ids
            intent['source_key'] = 'temporary:' + activation['id']
            intent['schedule']['due_at'] += 3600
            with open(path, 'w') as output:
                json.dump(intent, output)
            temporary = actor('loop-schedule', '--member-id', target, '--request-file', path)
            assert actor('loop-schedule-revoke', '--registration-id', temporary['registration']['id'])['state'] == 'revoked'
            with open(log_path, 'a') as log:
                log.write(json.dumps({'scheduled_by': activation['actor_id'], 'registration_id': registration_id}) + '\n')
            path = os.path.join(os.getcwd(), 'loop-outcome.json')
            with open(path, 'w') as output:
                json.dump({'kind': 'no_actionable_work'}, output)
            actor('loop-finish', '--outcome-file', path)
        if mode == 'role-wait':
            context = actor('loop-context')
            path = os.path.join(os.getcwd(), 'loop-outcome.json')
            outcome = {'kind':'waiting', 'wait_reason':'due_time', 'continuation':{'due_at':int(time.time()), 'task_id':None}}
            if any(source['input']['kind'] == 'continuation' for source in context['sources']):
                outcome = {'kind':'no_actionable_work'}
            with open(path, 'w') as output:
                json.dump(outcome, output)
            actor('loop-finish', '--outcome-file', path)
        if mode in ['finish', 'mcp', 'mem', 'role-pin', 'apps']:
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
        let state = crate::api::team_tests::build_test_state().await;
        Self::with_state(state, mode, endpoint).await
    }

    async fn with_state(
        mut state: crate::state::AppState,
        mode: &str,
        endpoint: Option<&str>,
    ) -> Self {
        let has_model: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('agents') WHERE name = 'runtime_model')",
        )
        .fetch_one(&state.db)
        .await
        .unwrap();
        if !has_model {
            sqlx::query("ALTER TABLE agents ADD COLUMN runtime_model TEXT")
                .execute(&state.db)
                .await
                .unwrap();
        }
        let directory =
            std::env::temp_dir().join(format!("agenthub-loop-provider-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let program = directory.join("claude-agent-acp");
        std::fs::write(&program, PROVIDER).unwrap();
        if mode == "mem" {
            std::fs::write(directory.join("mem_provider.py"), mem::provider::SCRIPT).unwrap();
        }
        if mode == "apps" {
            std::fs::write(directory.join("app_provider.py"), apps::PROVIDER).unwrap();
        }
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
        }
        if endpoint.is_some() || mode == "apps" {
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

    async fn execute_pending(
        &self,
        actor: &str,
    ) -> agenthub_agent_domain::loop_runtime::LoopActivation {
        let store = LoopStore::new(self.state.db.clone());
        let id: String = sqlx::query_scalar("SELECT id FROM loop_activations WHERE actor_id = ? AND state = 'pending' ORDER BY id LIMIT 1")
            .bind(actor).fetch_one(&self.state.db).await.unwrap();
        let LoopAdmission::Admitted(reservation) = store
            .admit(
                &self.team_id,
                &id,
                self.state.agents.loop_owner_id(),
                Utc::now().timestamp(),
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
        tokio::time::timeout(
            Duration::from_secs(20),
            self.state
                .agents
                .execute_loop_activation(self.state.teams.clone(), reservation),
        )
        .await
        .unwrap()
        .unwrap();
        let activation = store.activation(&self.team_id, &id).await.unwrap().unwrap();
        let log = std::fs::read_to_string(self.directory.join("requests.jsonl")).unwrap();
        assert_eq!(activation.state, LoopActivationState::Finished, "{log}");
        assert!(
            store
                .reservation(&self.team_id, actor)
                .await
                .unwrap()
                .is_none()
        );
        activation
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

#[tokio::test]
async fn loop_work_provider_dispatch_survives_leader_exit_and_report_wakes_offline_leader() {
    let fixture = Fixture::new("handoff").await;
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
                source_key: "dispatch".into(),
                due_at: None,
                references: LoopSourceReferences::default(),
            },
            now,
        )
        .await
        .unwrap();
    let first = fixture.execute_pending("planner").await;
    assert!(
        store
            .reservation(&fixture.team_id, "worker")
            .await
            .unwrap()
            .is_none()
    );
    let worker = fixture.execute_pending("worker").await;
    let sources = store.triggers(&fixture.team_id, &worker.id).await.unwrap();
    assert_eq!(sources.len(), 2);
    assert!(sources.iter().all(
        |source| source.input.references.scheduling_activation_id.as_deref()
            == Some(first.id.as_str())
    ));
    assert!(
        store
            .reservation(&fixture.team_id, "planner")
            .await
            .unwrap()
            .is_none()
    );
    let report = fixture.execute_pending("planner").await;
    assert_ne!(first.session_id, report.session_id);
    assert_eq!(first.mailbox_run_id, report.mailbox_run_id);
    let sources = store.triggers(&fixture.team_id, &report.id).await.unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(
        sources[0]
            .input
            .references
            .scheduling_activation_id
            .as_deref(),
        Some(worker.id.as_str())
    );
    let pending: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM loop_activations WHERE state = 'pending'")
            .fetch_one(&fixture.state.db)
            .await
            .unwrap();
    assert_eq!(pending, 0);
    let status: String =
        sqlx::query_scalar("SELECT status FROM team_tasks WHERE title = 'Offline review'")
            .fetch_one(&fixture.state.db)
            .await
            .unwrap();
    assert_eq!(status, "completed");
    let transcript = std::fs::read_to_string(fixture.directory.join("requests.jsonl")).unwrap();
    assert!(transcript.contains("worker_acceptance_denied"));
    assert!(transcript.contains("coordinator_accepted"));
    assert_eq!(transcript.matches("session/prompt").count(), 3);
    assert_eq!(transcript.matches("session/new").count(), 3);
    assert!(!transcript.contains("session/load"));
    fixture.close().await;
}

#[test]
fn loop_work_entry_prompt_is_a_bounded_versioned_recovery_pointer() {
    assert_eq!(LOOP_ENTRY_PROMPT_VERSION, "loop-entry-v6");
    assert!(LOOP_ENTRY_PROMPT.len() < 1500);
    for command in [
        "loop-context",
        "loop-source",
        "loop-schedule",
        "loop-finish",
    ] {
        assert!(LOOP_ENTRY_PROMPT.contains(command));
    }
}

#[tokio::test]
async fn loop_schedule_provider_cli_cycles_stop_under_each_members_durable_budget() {
    let fixture = Fixture::new("scheduling").await;
    let store = LoopStore::new(fixture.state.db.clone());
    let limits = LoopLimits {
        consecutive_no_progress: 2,
        ..LoopLimits::default()
    };
    for actor in ["planner", "worker"] {
        let policy = store
            .policy(&fixture.team_id, actor)
            .await
            .unwrap()
            .unwrap();
        store
            .configure(
                LoopPolicyUpdate {
                    actor_id: actor,
                    team_id: &fixture.team_id,
                    expected_revision: policy.revision,
                    state: LoopPolicyState::Enabled,
                    session_policy: LoopSessionPolicy::Fresh,
                    limits: &limits,
                },
                Utc::now().timestamp(),
            )
            .await
            .unwrap();
    }
    let mut previous = fixture.execute("schedule-cycle").await;
    assert_eq!(previous.state, LoopActivationState::Finished);
    for actor in ["planner", "worker", "planner"] {
        let firing = store
            .reconcile_schedules(Utc::now().timestamp())
            .await
            .unwrap();
        assert_eq!(firing.len(), 1);
        let detail = store
            .registration_detail(&fixture.team_id, &firing[0].registration_id, None, 1)
            .await
            .unwrap();
        assert_eq!(
            detail
                .registration
                .input
                .references
                .scheduling_activation_id
                .as_deref(),
            Some(previous.id.as_str())
        );
        let next = fixture.execute_pending(actor).await;
        assert_ne!(next.session_id, previous.session_id);
        previous = next;
    }
    let firing = store
        .reconcile_schedules(Utc::now().timestamp())
        .await
        .unwrap()
        .remove(0);
    assert_eq!(
        store
            .admit(
                &fixture.team_id,
                &firing.receipt.activation_id,
                fixture.state.agents.loop_owner_id(),
                Utc::now().timestamp()
            )
            .await
            .unwrap(),
        LoopAdmission::Deferred(
            agenthub_agent_domain::loop_runtime::LoopDeferralReason::NoProgressLimit
        )
    );
    for actor in ["planner", "worker"] {
        assert_eq!(
            store
                .policy(&fixture.team_id, actor)
                .await
                .unwrap()
                .unwrap()
                .no_progress_count,
            2
        );
        assert!(
            store
                .reservation(&fixture.team_id, actor)
                .await
                .unwrap()
                .is_none()
        );
    }
    let requests = std::fs::read_to_string(fixture.directory.join("requests.jsonl")).unwrap();
    assert_eq!(requests.matches("scheduled_by").count(), 4, "{requests}");
    assert_eq!(requests.matches("session/prompt").count(), 4, "{requests}");
    assert!(!requests.contains("cli_error"), "{requests}");
    fixture.close().await;
}
