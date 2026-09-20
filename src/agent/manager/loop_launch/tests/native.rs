use serde_json::{Value, json};

use super::*;

const PROVIDER: &str = r#"#!/usr/bin/env python3
import json, pathlib, subprocess, sys, time, uuid
root = pathlib.Path.cwd()
config = json.loads((root / 'native-fixture.json').read_text())
hello = config['handshake']
runtime = hello['runtime_id'] = str(uuid.uuid4())
native = str(uuid.uuid4())
sequence = 0
def emit(kind, payload):
    print(json.dumps({'type': kind, 'payload': payload}), flush=True)
def event(family, kind, payload, turn=None):
    global sequence
    sequence += 1
    operation = {'type':kind}
    if payload is not None:
        operation['payload'] = payload
    emit('event', {'runtime_id':runtime, 'session_id':native, 'event':{
        'event_id':'event-' + str(sequence), 'sequence':sequence, 'turn_id':turn,
        'provenance':{'session_id':native},
        'event':{'type':family, 'payload':operation}}})
def ack(request_id, turn=None):
    emit('ack', {'runtime_id':runtime, 'request_id':request_id, 'result':{
        'status':'accepted', 'session_id':native, 'turn_id':turn, 'last_sequence':sequence}})
emit('handshake', hello)
for line in sys.stdin:
    request = json.loads(line)
    with (root / 'native-requests.jsonl').open('a') as log:
        log.write(json.dumps(request) + '\n')
    if request['type'] == 'shutdown':
        rid = request['payload']['request_id']
        ack(rid)
        emit('shutdown_complete', {'runtime_id':runtime, 'request_id':rid})
        break
    envelope = request['payload']['envelope']
    rid = envelope['request_id']
    family = envelope['request']['type']
    operation = envelope['request']['payload']['type']
    body = envelope['request']['payload'].get('payload', {})
    if operation == 'create_session':
        if config['mode'] == 'pin-card':
            (root / 'native-starting').write_text(native)
            deadline = time.monotonic() + 10
            while not (root / 'native-release').exists():
                assert time.monotonic() < deadline
                time.sleep(0.01)
        ack(rid)
        event('session', 'created', {'session_id':native})
    elif family == 'prompt_source':
        assert body['scope'] == 'session' and body['layer'] == 'user'
        if config['mode'] == 'reject-source':
            emit('ack', {'runtime_id':runtime, 'request_id':rid, 'result':{'status':'rejected', 'code':'unsupported', 'message':'fixture'}})
            continue
        event('prompt_source', 'registered', {'source_id':body['source_id']})
        ack(rid)
    elif family == 'skill_source':
        event('skill', 'registered', {'source_id':body['source_id'], 'name':body['name']})
        ack(rid)
    elif operation == 'submit_user_prompt':
        turn = str(uuid.uuid4())
        event('session', 'turn_started', None, turn)
        ack(rid, turn)
        if config['mode'] == 'waiting':
            event('input', 'requested', {'pending':{'turn_id':turn, 'kind':{'type':'user','payload':{'question':'Choose scope', 'options':[], 'note':None}}}}, turn)
            event('session', 'turn_finished', {'reason':'awaiting_input'}, turn)
            (root / 'native-waiting').write_text(turn)
            continue
        if config['mode'] in ['finish', 'pin-card']:
            denied = subprocess.run([config['control'], 'actor', 'team-tasks', '--actor-id', 'native-child', '--json'], capture_output=True, text=True)
            assert denied.returncode != 0
            for command in ['loop-context', 'team-members', 'team-tasks', 'inbox']:
                result = subprocess.run([config['control'], 'actor', command, '--json'], capture_output=True, text=True)
                assert result.returncode == 0, result.stderr
            outcome = root / 'native-outcome.json'
            outcome.write_text(json.dumps({'kind':'no_actionable_work'}))
            result = subprocess.run([config['control'], 'actor', 'loop-finish', '--outcome-file', str(outcome), '--json'], capture_output=True, text=True)
            assert result.returncode == 0, result.stderr
        event('session', 'turn_finished', {'reason':'completed'}, turn)
    elif operation == 'cancel_current_turn':
        assert request['payload']['expected_turn_id'] == turn
        ack(rid, turn)
        event('input', 'discarded', {'waiting_turn':turn, 'reason':'cancelled'}, turn)
    else:
        raise AssertionError(operation)
"#;

async fn fixture(mode: &str) -> Fixture {
    let mut fixture = Fixture::new("no-outcome").await;
    let program = fixture.directory.join("native-provider");
    std::fs::write(&program, PROVIDER).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
    let wire: Value = serde_json::from_str(include_str!(
        "../../../../../crates/agenthub-rara/fixtures/stdio-v1.json"
    ))
    .unwrap();
    std::fs::write(
        fixture.directory.join("native-fixture.json"),
        json!({
            "mode":mode,
            "handshake":wire["frames"][0]["payload"],
            "control":crate::agenthub_binary::resolve_agenthub_binary_path().unwrap(),
        })
        .to_string(),
    )
    .unwrap();
    let mut config = (*fixture.state.agents.loop_app_config).clone();
    config.rara = Some(agenthub_config::RaraConfig {
        binary: Some(program.to_string_lossy().into_owned()),
        ..Default::default()
    });
    fixture.state.agents = Arc::new((*fixture.state.agents).clone().with_loop_app_config(config));
    sqlx::query(
        "UPDATE agents SET command = 'rara', args = '[]' WHERE id IN ('planner', 'worker')",
    )
    .execute(&fixture.state.db)
    .await
    .unwrap();
    fixture
}

#[tokio::test]
async fn native_loop_fresh_follow_up_pins_one_role_entry_and_keeps_mailbox_identity() {
    let fixture = fixture("finish").await;
    let first = fixture.execute("first-native").await;
    let second = fixture.execute("second-native").await;
    for activation in [&first, &second] {
        assert_eq!(activation.state, LoopActivationState::Finished);
        assert_eq!(activation.launch.as_ref().unwrap().provider_id, "rara");
        assert!(
            activation
                .launch
                .as_ref()
                .unwrap()
                .entry_prompt_version
                .contains(":worker:")
        );
    }
    assert_ne!(first.id, second.id);
    assert_ne!(first.session_id, second.session_id);
    assert_eq!(first.mailbox_run_id, second.mailbox_run_id);
    let requests: Vec<Value> =
        std::fs::read_to_string(fixture.directory.join("native-requests.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
    let registrations: Vec<_> = requests
        .iter()
        .filter(|request| request["payload"]["envelope"]["request"]["type"] == "prompt_source")
        .collect();
    assert_eq!(registrations.len(), 2);
    for (registration, activation) in registrations.iter().zip([&first, &second]) {
        let content =
            registration["payload"]["envelope"]["request"]["payload"]["payload"]["content"]
                .as_str()
                .unwrap();
        assert_eq!(
            content
                .matches("Run one bounded AgentHub activation.")
                .count(),
            1
        );
        assert_eq!(content.matches("You are a Team Worker").count(), 1);
        assert!(content.contains(&activation.id));
        assert!(content.contains("Native subagents execute within this outer activation"));
        assert!(!content.contains("You are a Team Coordinator"));
    }
    assert_ne!(
        registrations[0]["payload"]["runtime_id"],
        registrations[1]["payload"]["runtime_id"]
    );
    assert_ne!(
        registrations[0]["payload"]["envelope"]["provenance"]["session_id"],
        registrations[1]["payload"]["envelope"]["provenance"]["session_id"]
    );
    assert_eq!(
        requests
            .iter()
            .filter(
                |request| request["payload"]["envelope"]["request"]["payload"]["type"]
                    == "submit_user_prompt"
            )
            .count(),
        2
    );
    assert_eq!(
        requests
            .iter()
            .filter(
                |request| request["payload"]["envelope"]["request"]["payload"]["payload"]["name"]
                    == "team-loop-runtime"
            )
            .count(),
        2
    );
    fixture.close().await;
}

#[tokio::test]
async fn native_loop_terminal_turn_without_finish_is_interrupted_and_resume_is_rejected() {
    let fixture = fixture("no-outcome").await;
    let activation = fixture.execute("missing-outcome").await;
    assert_eq!(activation.state, LoopActivationState::Interrupted);
    assert!(activation.outcome.is_none());
    let team = fixture
        .state
        .teams
        .get_team(&fixture.team_id)
        .await
        .unwrap();
    let preflight = fixture
        .state
        .agents
        .loop_preflight(
            &fixture.team_id,
            &team.spec,
            "worker",
            LoopSessionPolicy::Resume,
        )
        .await
        .unwrap();
    assert!(!preflight.ready);
    assert!(preflight.blockers.contains(&"native_resume_unsupported"));
    assert!(
        preflight
            .warnings
            .contains(&"native_permissions_require_live_runtime")
    );
    fixture.close().await;
}

#[tokio::test]
async fn native_loop_cancellation_settles_a_waiting_turn_without_a_second_terminal_event() {
    let fixture = fixture("waiting").await;
    let (activation, ()) = tokio::join!(fixture.execute("waiting-native"), async {
        tokio::time::timeout(Duration::from_secs(10), async {
            while !fixture.directory.join("native-waiting").exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            let input = fixture.state.agents.inner.read().await["worker"]
                .input
                .clone();
            let AgentInput::Rara(runtime) = input else {
                panic!("native runtime");
            };
            assert!(!runtime.loop_turn_complete().await);
            fixture.state.agents.cancel_acp("worker").await.unwrap();
        })
        .await
        .unwrap();
    });
    assert_eq!(activation.state, LoopActivationState::Interrupted);
    assert!(activation.outcome.is_none());
    fixture.close().await;
}

#[tokio::test]
async fn native_loop_rejected_source_prevents_entry_and_cleans_the_owned_executor() {
    let fixture = fixture("reject-source").await;
    let reservation = fixture.admit("rejected-native-source").await;
    assert!(
        fixture
            .state
            .agents
            .execute_loop_activation(fixture.state.teams.clone(), reservation.clone())
            .await
            .is_err()
    );
    fixture
        .state
        .agents
        .fence_loop_reservation(&reservation)
        .await
        .unwrap();
    let log = std::fs::read_to_string(fixture.directory.join("native-requests.jsonl")).unwrap();
    assert!(!log.contains("submit_user_prompt"));
    assert_eq!(log.lines().count(), 2);
    assert!(
        LoopStore::new(fixture.state.db.clone())
            .reservation(&fixture.team_id, "worker")
            .await
            .unwrap()
            .is_none()
    );
    fixture.close().await;
}

#[tokio::test]
async fn native_loop_missing_source_method_fails_before_native_session_creation() {
    let fixture = fixture("no-outcome").await;
    let path = fixture.directory.join("native-fixture.json");
    let mut config: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    config["handshake"]["request_methods"]
        .as_array_mut()
        .unwrap()
        .retain(|method| method != "skill_source.register");
    std::fs::write(path, config.to_string()).unwrap();
    let reservation = fixture.admit("missing-source-method").await;
    assert!(
        fixture
            .state
            .agents
            .execute_loop_activation(fixture.state.teams.clone(), reservation.clone())
            .await
            .is_err()
    );
    fixture
        .state
        .agents
        .fence_loop_reservation(&reservation)
        .await
        .unwrap();
    assert!(!fixture.directory.join("native-requests.jsonl").exists());
    fixture.close().await;
}

async fn task_trigger(fixture: &Fixture, task_id: &str, key: &str) {
    LoopStore::new(fixture.state.db.clone())
        .accept_trigger(
            &LoopTriggerInput {
                actor_id: "worker".into(),
                team_id: fixture.team_id.clone(),
                kind: LoopTriggerKind::Operator,
                source_key: key.into(),
                due_at: None,
                references: LoopSourceReferences {
                    task_id: Some(task_id.into()),
                    ..Default::default()
                },
            },
            Utc::now().timestamp(),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn native_loop_pins_card_and_task_sources_before_start_and_reuses_task_prefix() {
    let fixture = fixture("pin-card").await;
    for task_id in ["task-one", "task-two"] {
        sqlx::query("INSERT INTO team_tasks(id, team_id, title, status, created_by_actor_id, assigned_member_id, context_json, created_at, updated_at) VALUES (?, ?, 'Review migration', 'open', 'planner', 'worker', '{\"summary\":\"Check rollback\",\"private\":\"omit-this-field\"}', 1, 1)")
            .bind(task_id).bind(&fixture.team_id).execute(&fixture.state.db).await.unwrap();
    }
    sqlx::query("UPDATE team_definitions SET spec_json = json_set(spec_json, '$.members[1].description', 'Review schema changes') WHERE id = ?")
        .bind(&fixture.team_id).execute(&fixture.state.db).await.unwrap();
    task_trigger(&fixture, "task-one", "task-first").await;
    let (first, ()) = tokio::join!(fixture.execute("first-card"), async {
        tokio::time::timeout(Duration::from_secs(10), async {
            while !fixture.directory.join("native-starting").exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        sqlx::query("UPDATE team_definitions SET spec_json = json_set(spec_json, '$.members[1].description', 'Review query plans') WHERE id = ?")
            .bind(&fixture.team_id).execute(&fixture.state.db).await.unwrap();
        sqlx::query("UPDATE team_tasks SET title = 'Clarified migration' WHERE id = 'task-one'")
            .execute(&fixture.state.db)
            .await
            .unwrap();
        std::fs::write(fixture.directory.join("native-release"), "continue").unwrap();
    });
    task_trigger(&fixture, "task-one", "task-reply").await;
    let follow_up = fixture.execute("follow-up-card").await;
    task_trigger(&fixture, "task-two", "task-new").await;
    let next = fixture.execute("next-card").await;
    for activation in [&first, &follow_up, &next] {
        assert_eq!(activation.state, LoopActivationState::Finished);
    }
    assert_ne!(
        first.launch.as_ref().unwrap().configuration_digest,
        follow_up.launch.as_ref().unwrap().configuration_digest
    );
    let frames: Vec<Value> =
        std::fs::read_to_string(fixture.directory.join("native-requests.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
    let registrations: Vec<_> = frames
        .iter()
        .filter(|frame| frame["payload"]["envelope"]["request"]["type"] == "prompt_source")
        .map(|frame| &frame["payload"]["envelope"]["request"]["payload"]["payload"])
        .collect();
    let cards: Vec<_> = registrations
        .iter()
        .filter(|source| source["source_id"] == "loop-activation-context-v2")
        .map(|source| source["content"].as_str().unwrap())
        .collect();
    assert_eq!(cards.len(), 3);
    assert!(cards[0].contains("Review schema changes"));
    assert!(!cards[0].contains("Review query plans"));
    assert!(cards[1].contains("Review query plans"));
    assert!(cards[0].contains("agenthub.a2a.discovery_card.v1"));
    let tasks: Vec<Value> = registrations
        .iter()
        .filter(|source| source["source_id"] == "loop-task-context-0")
        .map(|source| serde_json::from_str(source["content"].as_str().unwrap()).unwrap())
        .collect();
    assert_eq!(tasks.len(), 3);
    assert_eq!(tasks[0]["title"], "Review migration");
    assert_eq!(tasks[1]["title"], "Clarified migration");
    assert_eq!(tasks[0]["memory_prefix"], tasks[1]["memory_prefix"]);
    assert_ne!(tasks[0]["memory_prefix"], tasks[2]["memory_prefix"]);
    assert!(
        !serde_json::to_string(&tasks)
            .unwrap()
            .contains("omit-this-field")
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM loop_task_memory_prefixes")
        .fetch_one(&fixture.state.db)
        .await
        .unwrap();
    assert_eq!(count, 2);
    let child_members: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM agents WHERE id = 'native-child'")
            .fetch_one(&fixture.state.db)
            .await
            .unwrap();
    assert_eq!(child_members, 0);
    fixture.close().await;
}
