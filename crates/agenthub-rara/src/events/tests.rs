use super::*;
use crate::{RuntimeEvent, ServerFrame};

fn event(sequence: u64, family: &str, kind: &str, payload: Value) -> EventFrame {
    EventFrame {
        runtime_id: "runtime".into(),
        session_id: "session".into(),
        event: RuntimeEvent {
            event_id: format!("event-{sequence}"),
            sequence,
            provenance: json!({"session_id":null,"secret":"PRIVATE-MARKER"}),
            turn_id: Some("turn-1".into()),
            event: json!({"type":family,"payload":{"type":kind,"payload":payload}}),
        },
    }
}

fn projector() -> EventProjector {
    EventProjector::new("runtime", "session").unwrap()
}

fn conversation(projection: &EventProjection, index: usize) -> &Value {
    match &projection.history[index] {
        ProjectedHistory::Conversation(value) => value,
        ProjectedHistory::System(_) => panic!("expected conversation entry"),
    }
}

fn question(sequence: u64, turn: &str) -> EventFrame {
    let mut frame = event(
        sequence,
        "input",
        "requested",
        json!({"pending":{
            "turn_id":turn,"kind":{"type":"user","payload":{
                "question":"Which target?", "options":[["alpha","First target"]], "note":null
            }}
        }}),
    );
    frame.event.turn_id = Some(turn.into());
    frame
}

#[test]
fn pinned_readiness_fixture_uses_outer_ownership_without_trusting_provenance() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../fixtures/stdio-v1.json")).unwrap();
    let ServerFrame::Event(frame) = serde_json::from_value(fixture["frames"][2].clone()).unwrap()
    else {
        panic!("event fixture");
    };
    let mut projector = EventProjector::new(&frame.runtime_id, &frame.session_id).unwrap();
    let projection = projector.project(&frame).unwrap();
    assert_eq!(conversation(&projection, 0)["payload"]["plugin_count"], 0);
    assert_eq!(
        projection.safe_metadata["native_session_id"],
        "session-fixture"
    );
    assert!(projection.safe_metadata.get("provenance").is_none());
    assert!(matches!(projection.effect, EventEffect::None));
}

#[test]
fn fingerprint_ignores_object_order_but_covers_identity_and_content() {
    let frame = event(
        1,
        "tool",
        "use",
        json!({"call_id":"call","name":"bash","input":{"b":2,"a":1}}),
    );
    let original = frame.fingerprint().unwrap();
    let mut ordered = frame.clone();
    ordered.event.event["payload"]["payload"]["input"] =
        serde_json::from_str(r#"{"a":1,"b":2}"#).unwrap();
    assert_eq!(original, ordered.fingerprint().unwrap());
    for field in [
        "runtime",
        "session",
        "id",
        "sequence",
        "turn",
        "payload",
        "provenance",
    ] {
        let mut altered = frame.clone();
        match field {
            "runtime" => altered.runtime_id = "other".into(),
            "session" => altered.session_id = "other".into(),
            "id" => altered.event.event_id = "other".into(),
            "sequence" => altered.event.sequence += 1,
            "turn" => altered.event.turn_id = None,
            "payload" => altered.event.event["payload"]["payload"]["input"]["a"] = json!(2),
            _ => altered.event.provenance = json!({}),
        }
        assert_ne!(original, altered.fingerprint().unwrap(), "{field}");
    }
}

#[test]
fn streaming_chunks_follow_contiguous_kind_and_turn_boundaries() {
    let mut projector = projector();
    let first = projector
        .project(&event(1, "assistant", "text_delta", json!("a")))
        .unwrap();
    let second = projector
        .project(&event(2, "assistant", "text_delta", json!("b")))
        .unwrap();
    assert_eq!(
        conversation(&first, 0)["message_id"],
        conversation(&second, 0)["message_id"]
    );
    assert_eq!(conversation(&second, 0)["chunk_index"], 1);
    assert_eq!(conversation(&second, 0)["text"], "b");
    let thought = projector
        .project(&event(3, "assistant", "thinking_delta", json!("why")))
        .unwrap();
    assert_ne!(
        conversation(&thought, 0)["message_id"],
        conversation(&second, 0)["message_id"]
    );
    assert_eq!(conversation(&thought, 0)["type"], "agent_thought");
    let mut next = event(4, "assistant", "thinking_delta", json!("next"));
    next.event.turn_id = Some("turn-2".into());
    let next = projector.project(&next).unwrap();
    assert_eq!(conversation(&next, 0)["chunk_index"], 0);
    projector
        .project(&event(5, "session", "turn_started", Value::Null))
        .unwrap();
    let restarted = projector
        .project(&event(6, "assistant", "text_delta", json!("new")))
        .unwrap();
    assert_eq!(conversation(&restarted, 0)["chunk_index"], 0);
    let complete = projector
        .project(&event(7, "assistant", "text", json!("whole")))
        .unwrap();
    assert_eq!(conversation(&complete, 0)["chunk"], false);
}

#[test]
fn discarded_projection_does_not_advance_committed_chunk_state() {
    let mut committed = projector();
    committed
        .project(&event(1, "assistant", "text_delta", json!("a")))
        .unwrap();
    let frame = event(2, "assistant", "text_delta", json!("b"));
    let mut discarded = committed.clone();
    discarded.project(&frame).unwrap();
    let retry = committed.project(&frame).unwrap();
    assert_eq!(conversation(&retry, 0)["chunk_index"], 1);
}

#[test]
fn tools_keep_identity_across_approval_turns_and_isolate_reused_or_missing_ids() {
    let mut projector = projector();
    let used = projector
        .project(&event(
            1,
            "tool",
            "use",
            json!({"call_id":"call","name":"bash","input":{"command":"ls"}}),
        ))
        .unwrap();
    let id = conversation(&used, 0)["id"].clone();
    let mut progress = event(
        2,
        "tool",
        "progress",
        json!({"call_id":"call","name":"bash","stream":"stderr","chunk":"line"}),
    );
    progress.event.turn_id = Some("answer-turn".into());
    let progress = projector.project(&progress).unwrap();
    assert_eq!(conversation(&progress, 0)["id"], id);
    assert_eq!(
        conversation(&progress, 0)["meta"]["terminal_output"]["data"],
        "line"
    );
    let result = projector
        .project(&event(
            3,
            "tool",
            "result",
            json!({"call_id":"call","name":"bash","content":"done","is_error":false}),
        ))
        .unwrap();
    assert_eq!(conversation(&result, 0)["id"], id);
    assert_eq!(conversation(&result, 0)["status"], "completed");
    let reused = projector
        .project(&event(
            4,
            "tool",
            "use",
            json!({"call_id":"call","name":"bash","input":{}}),
        ))
        .unwrap();
    assert_ne!(conversation(&reused, 0)["id"], id);
    let unidentified = projector
        .project(&event(
            5,
            "tool",
            "result",
            json!({"name":"bash","content":"unknown call","is_error":true}),
        ))
        .unwrap();
    assert_ne!(
        conversation(&unidentified, 0)["id"],
        conversation(&reused, 0)["id"]
    );
    assert_eq!(conversation(&unidentified, 0)["status"], "failed");
    assert!(
        projector
            .project(&event(
                6,
                "tool",
                "progress",
                json!({"call_id":"call","name":"other","stream":"stdout","chunk":"bad"})
            ))
            .is_err()
    );
}

#[test]
fn questions_carry_native_fences_and_old_answers_cannot_clear_new_question() {
    let mut projector = projector();
    let first = projector.project(&question(1, "old-turn")).unwrap();
    assert_eq!(
        conversation(&first, 0)["meta"]["native_input"],
        json!({"runtime_id":"runtime","session_id":"session","turn_id":"old-turn"})
    );
    assert_eq!(
        conversation(&first, 0)["raw_input"][0]["options"][0]["label"],
        "alpha"
    );
    assert!(matches!(first.effect, EventEffect::InputRequested(_)));
    let current = projector.project(&question(2, "new-turn")).unwrap();
    let old = projector
        .project(&event(
            3,
            "input",
            "answered",
            json!({"waiting_turn":"old-turn"}),
        ))
        .unwrap();
    assert!(old.history.is_empty());
    let answer = projector
        .project(&event(
            4,
            "input",
            "answered",
            json!({"waiting_turn":"new-turn"}),
        ))
        .unwrap();
    assert_eq!(
        conversation(&answer, 0)["id"],
        conversation(&current, 0)["id"]
    );
    assert_eq!(conversation(&answer, 0)["status"], "completed");
    projector.project(&question(5, "last-turn")).unwrap();
    let discarded = projector
        .project(&event(
            6,
            "input",
            "discarded",
            json!({"waiting_turn":"last-turn","reason":"shutdown"}),
        ))
        .unwrap();
    assert_eq!(conversation(&discarded, 0)["status"], "failed");
}

#[test]
fn approval_notice_is_not_a_live_input_request() {
    let mut projector = projector();
    let notice = projector
        .project(&event(
            1,
            "approval",
            "requested",
            json!({"approval_id":"call","kind":"shell"}),
        ))
        .unwrap();
    assert!(matches!(notice.effect, EventEffect::None));
    for (index, kind, body) in [
        (
            2,
            "shell",
            json!({"approval_id":"call","request":{"command":"ls"}}),
        ),
        (4, "plan", json!({"approval_id":"plan","plan":"Review"})),
    ] {
        let projected = projector
            .project(&event(
                index,
                "input",
                "requested",
                json!({"pending":{
                    "turn_id":"turn-1","kind":{"type":kind,"payload":body}
                }}),
            ))
            .unwrap();
        assert!(matches!(projected.effect, EventEffect::InputRequested(_)));
        assert_eq!(projected.history.len(), 1);
        let answer = projector
            .project(&event(
                index + 1,
                "input",
                "answered",
                json!({"waiting_turn":"turn-1"}),
            ))
            .unwrap();
        assert!(answer.history.is_empty());
    }
    let answered = projector
        .project(&event(
            6,
            "approval",
            "answered",
            json!({"approval_id":"call","approved":false}),
        ))
        .unwrap();
    assert!(matches!(
        answered.effect,
        EventEffect::ApprovalAnswered {
            approved: false,
            ..
        }
    ));
}

#[test]
fn snapshot_and_pending_turns_are_bound_to_owned_native_session() {
    let mut projector = projector();
    let snapshot = json!({"session_id":"session","phase":{"state":"awaiting_input","detail":{"turn_id":"turn-1"}},
    "generation":1,"last_sequence":1,"pending_input":{"turn_id":"turn-1","kind":{"type":"user","payload":{
        "question":"Next?","options":[],"note":null
    }}}});
    let frame = event(2, "session", "runtime_state", json!({"snapshot":snapshot}));
    let projection = projector.project(&frame).unwrap();
    assert!(matches!(projection.effect, EventEffect::Snapshot(_)));
    for (field, value) in [
        ("session_id", json!("foreign")),
        ("last_sequence", json!(3)),
        ("pending_input", Value::Null),
    ] {
        let mut invalid = frame.clone();
        invalid.event.event["payload"]["payload"]["snapshot"][field] = value;
        assert!(projector.project(&invalid).is_err(), "{field}");
    }
    let mut invalid = question(3, "waiting");
    invalid.event.turn_id = Some("foreign".into());
    assert!(projector.project(&invalid).is_err());
    invalid.session_id = "foreign".into();
    assert!(projector.project(&invalid).is_err());
}

#[test]
fn native_diagnostics_never_copy_raw_secrets_into_projected_history() {
    let cases = [
        (
            "memory",
            "records_queried",
            json!({"query":"PRIVATE-MARKER","records":[{"content":"PRIVATE-MARKER"}]}),
        ),
        (
            "hook",
            "command_output",
            json!({"stdout":"PRIVATE-MARKER","stderr":"PRIVATE-MARKER","ok":false,"timed_out":true,"exit_code":1}),
        ),
        (
            "skill",
            "catalogue",
            json!({"skills":[{"content":"PRIVATE-MARKER"}]}),
        ),
        (
            "prompt_source",
            "dropped",
            json!({"source_id":"source","reason":"PRIVATE-MARKER"}),
        ),
        (
            "context",
            "observability_updated",
            json!({"view":{"secret":"PRIVATE-MARKER"}}),
        ),
        (
            "mcp",
            "status_load_failed",
            json!({"message":"PRIVATE-MARKER"}),
        ),
        (
            "warning",
            "runtime_warning",
            json!({"message":"PRIVATE-MARKER"}),
        ),
        (
            "error",
            "runtime_error",
            json!({"message":"PRIVATE-MARKER","recoverable":false}),
        ),
        (
            "session",
            "compacted",
            json!({"count":1,"before_tokens":9,"after_tokens":2,"summary":"PRIVATE-MARKER","recent_files":["PRIVATE-MARKER"]}),
        ),
    ];
    for (index, (family, kind, payload)) in cases.into_iter().enumerate() {
        let projected = projector()
            .project(&event(index as u64 + 1, family, kind, payload))
            .unwrap();
        assert!(
            !projected
                .safe_metadata
                .to_string()
                .contains("PRIVATE-MARKER")
        );
        for history in projected.history {
            let text = match history {
                ProjectedHistory::Conversation(v) => v.to_string(),
                ProjectedHistory::System(s) => s,
            };
            assert!(!text.contains("PRIVATE-MARKER"), "{family}/{kind}");
        }
    }
    let assistant = projector()
        .project(&event(20, "assistant", "text", json!("PRIVATE-MARKER")))
        .unwrap();
    assert_eq!(conversation(&assistant, 0)["text"], "PRIVATE-MARKER");
    assert!(
        !assistant
            .safe_metadata
            .to_string()
            .contains("PRIVATE-MARKER")
    );
}

#[test]
fn native_turn_outcomes_and_plan_progress_keep_their_meaning() {
    let mut projector = projector();
    let started = projector
        .project(&event(1, "session", "turn_started", Value::Null))
        .unwrap();
    assert!(matches!(started.effect, EventEffect::TurnStarted { .. }));
    let finished = projector
        .project(&event(
            2,
            "session",
            "turn_finished",
            json!({"reason":"needs_clarification"}),
        ))
        .unwrap();
    assert!(
        matches!(finished.effect,EventEffect::TurnEnded { outcome:TurnEnd::Finished { reason:Some(reason) },.. } if reason == "needs_clarification")
    );
    for (i, kind, status) in [
        (3, "turn_cancelled", "cancelled"),
        (4, "turn_interrupted", "cancelled"),
        (5, "turn_failed", "error"),
    ] {
        let payload = if kind == "turn_failed" {
            json!({"reason":"private"})
        } else {
            Value::Null
        };
        let projected = projector
            .project(&event(i, "session", kind, payload))
            .unwrap();
        assert_eq!(conversation(&projected, 0)["status"], status);
        assert!(matches!(projected.effect, EventEffect::TurnEnded { .. }));
    }
    let plan = projector
        .project(&event(
            6,
            "plan",
            "updated",
            json!({"steps":[{"step":"Inspect","status":"in_progress"}],"explanation":"Checking"}),
        ))
        .unwrap();
    assert_eq!(
        conversation(&plan, 0)["plan"]["entries"][0]["status"],
        "in_progress"
    );
    let mut no_turn = event(7, "session", "turn_started", Value::Null);
    no_turn.event.turn_id = None;
    assert!(projector.project(&no_turn).is_err());
    let waiting = projector
        .project(&event(
            8,
            "session",
            "turn_finished",
            json!({"reason":"awaiting_input"}),
        ))
        .unwrap();
    assert_eq!(conversation(&waiting, 0)["status"], "waiting_permission");
}

#[test]
fn unknown_or_malformed_events_fail_with_fixed_error_categories() {
    for frame in [
        event(
            1,
            "new_family",
            "secret",
            json!({"message":"PRIVATE-MARKER"}),
        ),
        event(1, "mcp", "new_kind", Value::Null),
        event(1, "tool", "result", json!({"content":"PRIVATE-MARKER"})),
        event(1, "extension", "readiness_updated", json!({"snapshot":{}})),
    ] {
        let error = projector()
            .project(&frame)
            .err()
            .expect("reject malformed event");
        assert!(!error.to_string().contains("PRIVATE-MARKER"));
    }
}
