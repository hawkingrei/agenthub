use agenthub_config::RaraSessionPolicy;
use agenthub_db::native_sessions::NativeSessionStore;
use serde_json::json;

use super::*;

mod native;

const PEER: &str = r#"#!/usr/bin/env python3
import json, pathlib, sys, uuid
root = pathlib.Path.cwd()
hello = json.loads((root / 'continuity-handshake.json').read_text())
runtime = hello['runtime_id'] = str(uuid.uuid4())
native, sequence = None, 0
def emit(kind, payload):
    print(json.dumps({'type':kind,'payload':payload}), flush=True)
def ack(rid):
    emit('ack',{'runtime_id':runtime,'request_id':rid,'result':{
        'status':'accepted','session_id':native,'turn_id':None,'last_sequence':sequence}})
emit('handshake',hello)
for line in sys.stdin:
    request = json.loads(line)
    with (root / 'continuity-requests.jsonl').open('a') as log:
        log.write(json.dumps(request) + '\n')
    if request['type'] == 'shutdown':
        rid = request['payload']['request_id']
        ack(rid)
        emit('shutdown_complete',{'runtime_id':runtime,'request_id':rid})
        break
    envelope = request['payload']['envelope']
    rid, operation = envelope['request_id'], envelope['request']['payload']['type']
    if operation == 'create_session':
        native = str(uuid.uuid4())
        (root / 'conversation').write_text(native)
    elif operation == 'resume_session':
        native = envelope['request']['payload']['payload']['session_id']
        assert native == (root / 'conversation').read_text()
    else:
        raise AssertionError(operation)
    if (root / 'drop-opening').exists():
        sys.exit(0)
    sequence += 1
    emit('event',{'runtime_id':runtime,'session_id':native,'event':{
        'event_id':'event-1','sequence':sequence,'provenance':{'session_id':native},
        'event':{'type':'session','payload':{'type':'created','payload':{'session_id':native}}}}})
    ack(rid)
"#;

async fn fixture() -> Fixture {
    let mut fixture = Fixture::new("normal").await;
    std::fs::write(fixture.directory.join("runtime fixture"), PEER).unwrap();
    let handshake = json!({
        "protocol_version":1,"runtime_version":"fixture","runtime_id":"replaced",
        "transport":"stdio-jsonl","request_families":["session","input","server"],
        "request_methods":["session.create","session.resume","session.query_state",
            "session.query_recovery","session.resolve_recovery","session.evaluate_reentry",
            "session.cancel","session.interrupt","input.submit_prompt","input.submit_follow_up",
            "input.answer_user","input.answer_plan","input.answer_shell","server.shutdown"],
        "event_families":["session","input","assistant","tool","approval","plan","warning","error"],
        "capabilities":{"graceful_shutdown":true,"approval_persistence":true,
            "replay":{"lifetime":"unavailable"},"request_receipts":{"lifetime":"runtime","max_requests":128}}
    });
    std::fs::write(
        fixture.directory.join("continuity-handshake.json"),
        handshake.to_string(),
    )
    .unwrap();
    set_policy(&mut fixture, RaraSessionPolicy::Resume);
    fixture
}

fn set_policy(fixture: &mut Fixture, policy: RaraSessionPolicy) {
    let mut config = (*fixture.manager.loop_app_config).clone();
    config.rara.as_mut().unwrap().standalone_session_policy = Some(policy);
    fixture.manager = fixture.manager.clone().with_loop_app_config(config);
}

fn requests(fixture: &Fixture) -> Vec<Value> {
    std::fs::read_to_string(fixture.directory.join("continuity-requests.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn openings(fixture: &Fixture) -> Vec<Value> {
    requests(fixture)
        .into_iter()
        .filter(|request| request["type"] == "control")
        .collect()
}

#[tokio::test]
async fn standalone_disconnected_start_keeps_cleanup_owned_until_spawn_settles() {
    #[derive(Debug)]
    struct ControlledFailure {
        entered: tokio::sync::Notify,
        release: tokio::sync::Notify,
    }
    #[async_trait::async_trait]
    impl AgentExecutor for ControlledFailure {
        async fn spawn_process(
            &self,
            request: LocalExecutionRequest,
        ) -> anyhow::Result<SpawnedLocalProcess> {
            assert!(request.guard_descendants && request.cleanup_witness.is_some());
            self.entered.notify_one();
            self.release.notified().await;
            drop(request);
            anyhow::bail!("controlled native spawn failure")
        }
    }
    let mut fixture = fixture().await;
    let executor = Arc::new(ControlledFailure {
        entered: Default::default(),
        release: Default::default(),
    });
    fixture.manager.local_executor = executor.clone();
    let manager = fixture.manager.clone();
    let agent = fixture.agent_id.clone();
    let caller = tokio::spawn(async move { manager.start_agent(&agent).await });
    executor.entered.notified().await;
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    let store = NativeSessionStore::new(fixture.manager.db.clone());
    assert!(
        store
            .active_owner(&fixture.agent_id)
            .await
            .unwrap()
            .is_some()
    );
    executor.release.notify_one();
    tokio::time::timeout(Duration::from_secs(5), async {
        while fixture
            .manager
            .starting
            .lock()
            .await
            .contains(&fixture.agent_id)
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        store
            .active_owner(&fixture.agent_id)
            .await
            .unwrap()
            .is_none()
    );
    fixture.finish().await;
}

#[tokio::test]
async fn standalone_resume_preserves_conversation_and_rejects_stale_owner() {
    let fixture = fixture().await;
    let store = NativeSessionStore::new(fixture.manager.db.clone());
    let first = fixture
        .manager
        .start_agent(&fixture.agent_id)
        .await
        .unwrap();
    let before = store
        .active_owner(&fixture.agent_id)
        .await
        .unwrap()
        .unwrap()
        .owner;
    let old = fixture.runtime().await;
    assert!(
        fixture
            .manager
            .clear_persistent_session(&fixture.agent_id, "rara")
            .await
            .is_err()
    );
    fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
    assert!(
        store
            .active_owner(&fixture.agent_id)
            .await
            .unwrap()
            .is_none()
    );
    let second = fixture
        .manager
        .start_agent(&fixture.agent_id)
        .await
        .unwrap();
    let after = store
        .active_owner(&fixture.agent_id)
        .await
        .unwrap()
        .unwrap()
        .owner;
    assert_ne!(first, second);
    assert_eq!(after.generation, before.generation + 1);
    assert!(
        old.send_input("stale", Some("stale"), None, None)
            .await
            .is_err()
    );
    assert!(old.query_recovery().await.is_err());
    assert!(old.stop_turn(false).await.is_err());
    let frames = openings(&fixture);
    assert_eq!(frames.len(), 2);
    assert_eq!(
        frames[0]["payload"]["envelope"]["request"]["payload"]["type"],
        "create_session"
    );
    assert_eq!(
        frames[1]["payload"]["envelope"]["request"]["payload"]["type"],
        "resume_session"
    );
    fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
    fixture
        .manager
        .clear_persistent_session(&fixture.agent_id, "rara")
        .await
        .unwrap();
    fixture
        .manager
        .start_agent(&fixture.agent_id)
        .await
        .unwrap();
    assert_eq!(
        store
            .active_owner(&fixture.agent_id)
            .await
            .unwrap()
            .unwrap()
            .owner
            .generation,
        3
    );
    assert_eq!(
        openings(&fixture)[2]["payload"]["envelope"]["request"]["payload"]["type"],
        "create_session"
    );
    fixture.finish().await;
}

#[tokio::test]
async fn standalone_resume_requires_full_recovery_before_opening() {
    for missing in [
        "session.resume",
        "session.query_recovery",
        "session.resolve_recovery",
        "session.evaluate_reentry",
        "approval_persistence",
    ] {
        let fixture = fixture().await;
        let path = fixture.directory.join("continuity-handshake.json");
        let mut handshake: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        if missing == "approval_persistence" {
            handshake["capabilities"][missing] = json!(false);
        } else {
            handshake["request_methods"]
                .as_array_mut()
                .unwrap()
                .retain(|method| method != missing);
        }
        std::fs::write(path, handshake.to_string()).unwrap();
        assert!(
            fixture
                .manager
                .start_agent(&fixture.agent_id)
                .await
                .is_err(),
            "{missing}"
        );
        assert!(openings(&fixture).is_empty(), "{missing}");
        assert!(
            NativeSessionStore::new(fixture.manager.db.clone())
                .active_owner(&fixture.agent_id)
                .await
                .unwrap()
                .is_none()
        );
        fixture.assert_clean().await;
        fixture.finish().await;
    }
}

#[tokio::test]
async fn standalone_uncertain_opening_never_falls_back_to_fresh() {
    let mut fixture = fixture().await;
    std::fs::write(fixture.directory.join("drop-opening"), "drop").unwrap();
    assert!(
        fixture
            .manager
            .start_agent(&fixture.agent_id)
            .await
            .is_err()
    );
    std::fs::remove_file(fixture.directory.join("drop-opening")).unwrap();
    assert!(
        fixture
            .manager
            .start_agent(&fixture.agent_id)
            .await
            .is_err()
    );
    assert_eq!(openings(&fixture).len(), 1);
    set_policy(&mut fixture, RaraSessionPolicy::Fresh);
    fixture
        .manager
        .start_agent(&fixture.agent_id)
        .await
        .unwrap();
    assert_eq!(openings(&fixture).len(), 2);
    fixture.finish().await;
}

#[tokio::test]
async fn standalone_closed_receipt_repairs_binding_but_configuration_changes_block() {
    let mut fixture = fixture().await;
    fixture
        .manager
        .start_agent(&fixture.agent_id)
        .await
        .unwrap();
    fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
    sqlx::query("UPDATE native_standalone_conversations SET state = 'opening', native_session_id = NULL WHERE agent_id = ?")
        .bind(&fixture.agent_id).execute(&fixture.manager.db).await.unwrap();
    fixture
        .manager
        .start_agent(&fixture.agent_id)
        .await
        .unwrap();
    assert_eq!(
        openings(&fixture)[1]["payload"]["envelope"]["request"]["payload"]["type"],
        "resume_session"
    );
    fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
    let mut config = (*fixture.manager.loop_app_config).clone();
    config.rara.as_mut().unwrap().default_model = Some("changed-model".into());
    fixture.manager = fixture.manager.clone().with_loop_app_config(config);
    assert!(
        fixture
            .manager
            .start_agent(&fixture.agent_id)
            .await
            .is_err()
    );
    assert_eq!(openings(&fixture).len(), 2);
    fixture.finish().await;
}

#[tokio::test]
async fn standalone_replacement_requires_cleanup_even_after_session_exit_marking() {
    let fixture = fixture().await;
    let store = NativeSessionStore::new(fixture.manager.db.clone());
    let owner = store
        .reserve(&fixture.agent_id, "old-local", "old-daemon", 100)
        .await
        .unwrap();
    let witness = crate::executor_guardian::CleanupWitness::prepare_standalone(
        fixture.manager.event_dbs.base_dir(),
        &owner,
    )
    .unwrap();
    store.authorize_spawn(&owner, 101).await.unwrap();
    sqlx::query("INSERT INTO agent_sessions(id, agent_id, status, started_at, ended_at) VALUES ('old-local', ?, 'exited', 101, 102)")
        .bind(&fixture.agent_id).execute(&fixture.manager.db).await.unwrap();
    assert!(
        fixture
            .manager
            .start_agent(&fixture.agent_id)
            .await
            .is_err()
    );
    assert!(openings(&fixture).is_empty());
    assert_eq!(
        store
            .active_owner(&fixture.agent_id)
            .await
            .unwrap()
            .unwrap()
            .owner,
        owner
    );
    // The prepared witness proves that no child passed the guardian startup fence.
    drop(witness);
    fixture
        .manager
        .start_agent(&fixture.agent_id)
        .await
        .unwrap();
    assert_eq!(
        store
            .active_owner(&fixture.agent_id)
            .await
            .unwrap()
            .unwrap()
            .owner
            .generation,
        2
    );
    fixture.finish().await;
}
