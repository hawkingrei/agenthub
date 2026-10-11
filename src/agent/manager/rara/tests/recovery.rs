use agenthub_rara::RecoveryTarget;
use serde_json::json;

use super::{input::output, *};
use crate::agent::AgentSendInputError;

const PEER: &str = r#"#!/usr/bin/env python3
import json, pathlib, sys
root = pathlib.Path.cwd()
hello = json.loads((root / 'recovery-handshake.json').read_text())
runtime, native = hello['runtime_id'], 'recovery-session'
sequence, waiting, blocked, resolution = 0, None, None, None
def emit(kind, payload):
    print(json.dumps({'type':kind, 'payload':payload}), flush=True)
def event(family, kind, payload=None, turn=None):
    global sequence
    sequence += 1
    operation = {'type':kind}
    if payload is not None:
        operation['payload'] = payload
    emit('event', {'runtime_id':runtime, 'session_id':native, 'event':{
        'event_id':'event-' + str(sequence), 'sequence':sequence, 'turn_id':turn,
        'provenance':{'session_id':native,'controller':'runtime','trust':'trusted','authorship':'runtime'},
        'event':{'type':family,'payload':operation}}})
def ack(rid, turn=None):
    emit('ack', {'runtime_id':runtime, 'request_id':rid, 'result':{
        'status':'accepted','session_id':native,'turn_id':turn,'last_sequence':sequence}})
def recovery():
    event('session','recovery_state',{'state':{
        'waiting_turn_id':waiting,'blocked':blocked,'decisions':[],'last_resolution':resolution}})
emit('handshake', hello)
for line in sys.stdin:
    request = json.loads(line)
    if request['type'] == 'shutdown':
        rid = request['payload']['request_id']
        ack(rid)
        emit('shutdown_complete',{'runtime_id':runtime,'request_id':rid})
        break
    envelope = request['payload']['envelope']
    rid, operation = envelope['request_id'], envelope['request']['payload']['type']
    if operation == 'create_session':
        event('session','created',{'session_id':native})
        ack(rid)
        continue
    with (root / 'requests.jsonl').open('a') as log:
        log.write(json.dumps(request) + '\n')
    if operation == 'submit_user_prompt':
        assert not waiting and not blocked
        waiting = 'turn-' + str(sequence)
        event('session','turn_started',turn=waiting)
        event('input','requested',{'pending':{'turn_id':waiting,'kind':{
            'type':'user','payload':{'question':'Choose scope','options':[],'note':None}}}}, waiting)
        event('session','turn_finished',{'reason':'awaiting_input'},waiting)
        ack(rid,waiting)
    elif operation == 'cancel_current_turn':
        assert request['payload']['expected_turn_id'] == waiting
        cancelled = waiting
        blocked = {'recovery_id':'cancelled-wait','turn_id':waiting,'reason':'pending_cancelled'}
        waiting = None
        event('input','discarded',{'waiting_turn':cancelled,'reason':'cancelled'},cancelled)
        ack(rid,cancelled)
    elif operation == 'query_recovery':
        recovery()
        ack(rid)
    elif operation == 'resolve_recovery':
        body = envelope['request']['payload']['payload']
        assert blocked and blocked['recovery_id'] == body['recovery_id']
        resolution, blocked = body, None
        recovery()
        ack(rid)
    else:
        raise AssertionError(operation)
"#;

#[tokio::test]
async fn cancelled_standalone_wait_requires_observed_recovery_before_new_input() {
    let fixture = Fixture::new("normal").await;
    std::fs::write(fixture.directory.join("runtime fixture"), PEER).unwrap();
    // This peer owns its capabilities; real captured wire data stays in the protocol crate.
    let handshake = json!({
        "protocol_version": agenthub_rara::PROTOCOL_VERSION,
        "runtime_version": "fixture",
        "runtime_id": "recovery-runtime",
        "transport": agenthub_rara::TRANSPORT,
        "request_families": ["session", "input", "server"],
        "request_methods": [
            "session.create", "session.query_state", "session.query_recovery",
            "session.resolve_recovery", "session.cancel", "session.interrupt",
            "input.submit_prompt", "input.submit_follow_up", "input.answer_user",
            "input.answer_plan", "input.answer_shell", "server.shutdown"
        ],
        "event_families": ["session", "input", "assistant", "tool", "approval", "plan", "warning", "error"],
        "capabilities": {
            "graceful_shutdown": true,
            "approval_persistence": true,
            "replay": {"lifetime":"unavailable"},
            "request_receipts": {"lifetime":"runtime", "max_requests":128}
        }
    });
    std::fs::write(
        fixture.directory.join("recovery-handshake.json"),
        handshake.to_string(),
    )
    .unwrap();
    let session = fixture
        .manager
        .start_agent(&fixture.agent_id)
        .await
        .unwrap();
    let runtime = fixture.runtime().await;
    let mut receiver = fixture
        .manager
        .subscribe_output(&fixture.agent_id)
        .await
        .unwrap();
    runtime
        .send_input("Ask before proceeding", Some("first"), None, None)
        .await
        .unwrap();
    output(&mut receiver, "tool_call").await;
    runtime.stop_turn(false).await.unwrap();
    output(&mut receiver, "tool_call_update").await;
    let error = runtime
        .send_input("Do not skip recovery", Some("blocked-input"), None, None)
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<AgentSendInputError>(),
        Some(AgentSendInputError::NativeRecoveryRequired)
    ));
    let current = fixture
        .manager
        .query_native_recovery(&fixture.agent_id, &session)
        .await
        .unwrap();
    assert_eq!(
        current.recovery.blocked.as_ref().unwrap().recovery_id,
        "cancelled-wait"
    );
    assert_eq!(current.local_session_id, session);
    fixture
        .manager
        .reconcile_native_recovery(
            &fixture.agent_id,
            &session,
            RecoveryTarget {
                runtime_id: current.runtime_id,
                session_id: current.session_id,
                recovery_id: "cancelled-wait".into(),
            },
            "Cancelled pending input; no action was admitted".into(),
        )
        .await
        .unwrap();
    let requests = fixture.input_requests();
    assert!(
        !requests
            .iter()
            .any(|request| request["payload"]["envelope"]["request_id"] == "blocked-input")
    );
    assert_eq!(
        requests
            .iter()
            .filter(
                |request| request["payload"]["envelope"]["request"]["payload"]["type"]
                    == "submit_user_prompt"
            )
            .count(),
        1
    );
    runtime
        .send_input("Proceed explicitly", Some("next"), None, None)
        .await
        .unwrap();
    let current = fixture
        .manager
        .query_native_recovery(&fixture.agent_id, &session)
        .await
        .unwrap();
    assert!(current.recovery.blocked.is_none());
    assert!(current.recovery.waiting_turn_id.is_some());
    assert_eq!(
        serde_json::to_value(current.recovery.last_resolution).unwrap()["recovery_id"],
        json!("cancelled-wait")
    );
    fixture.finish().await;
}
