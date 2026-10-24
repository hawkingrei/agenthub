use super::*;
use crate::{RuntimeEvent, SemanticGuardDecision};

fn frame(sequence: u64, family: &str, kind: &str, payload: Value) -> EventFrame {
    EventFrame {
        runtime_id: "runtime".into(),
        session_id: "session".into(),
        event: RuntimeEvent {
            event_id: format!("event-{sequence}"),
            sequence,
            provenance: json!({"session_id":"session","controller":"runtime","trust":"trusted","authorship":"runtime"}),
            turn_id: Some("turn".into()),
            event: json!({"type":family,"payload":{"type":kind,"payload":payload}}),
        },
    }
}

fn started() -> EventProjector {
    let mut projector = EventProjector::new("runtime", "session").unwrap();
    projector
        .project(&frame(1, "session", "turn_started", Value::Null))
        .unwrap();
    projector
}

fn decision(value: Value) -> EventFrame {
    frame(2, "semantic_guard", "decided", json!({"decision":value}))
}

fn mismatch() -> EventFrame {
    decision(json!({"outcome":"mismatch","reason":"PRIVATE-REASON"}))
}

#[test]
fn semantic_guard_declines_require_a_normal_terminal_and_keep_reason_out_of_telemetry() {
    for value in [
        json!({"outcome":"mismatch","reason":"PRIVATE-REASON"}),
        json!({"outcome":"needs_clarification","reason":"PRIVATE-REASON","question":"Which task?"}),
    ] {
        let mut projector = started();
        let projected = projector.project(&decision(value.clone())).unwrap();
        assert!(matches!(projected.effect, EventEffect::None));
        assert!(
            !projected
                .safe_metadata
                .to_string()
                .contains("PRIVATE-REASON")
        );
        let ProjectedHistory::Conversation(history) = &projected.history[0] else {
            panic!("missing scoped guard history")
        };
        assert!(history.to_string().contains("PRIVATE-REASON"));
        let terminal = projector
            .project(&frame(
                3,
                "session",
                "turn_finished",
                json!({"reason":"done"}),
            ))
            .unwrap();
        let EventEffect::TurnEnded {
            semantic: Some(outcome),
            ..
        } = terminal.effect
        else {
            panic!("missing terminal semantic result")
        };
        assert_eq!(serde_json::to_value(outcome).unwrap(), value);
    }
}

#[test]
fn semantic_guard_compatible_and_unavailable_continue_worker_without_semantic_finish() {
    for guard in [
        decision(json!({"outcome":"compatible"})),
        frame(
            2,
            "semantic_guard",
            "unavailable",
            json!({"reason":"timeout"}),
        ),
        frame(
            2,
            "semantic_guard",
            "unavailable",
            json!({"reason":"provider"}),
        ),
        frame(
            2,
            "semantic_guard",
            "unavailable",
            json!({"reason":"invalid_response"}),
        ),
    ] {
        let mut projector = started();
        projector.project(&guard).unwrap();
        projector
            .project(&frame(3, "assistant", "text", json!("result")))
            .unwrap();
        let end = projector
            .project(&frame(
                4,
                "session",
                "turn_finished",
                json!({"reason":"mismatch"}),
            ))
            .unwrap();
        assert!(matches!(
            end.effect,
            EventEffect::TurnEnded { semantic: None, .. }
        ));
    }
    let end = started()
        .project(&frame(
            2,
            "session",
            "turn_finished",
            json!({"reason":"needs_clarification"}),
        ))
        .unwrap();
    assert!(matches!(
        end.effect,
        EventEffect::TurnEnded { semantic: None, .. }
    ));
}

#[test]
fn semantic_guard_rejects_ambiguous_order_identity_and_worker_activity() {
    assert!(
        EventProjector::new("runtime", "session")
            .unwrap()
            .project(&mismatch())
            .is_err()
    );
    let mut declined = started();
    declined.project(&mismatch()).unwrap();
    for later in [
        mismatch(),
        decision(json!({"outcome":"compatible"})),
        frame(3, "session", "turn_started", Value::Null),
        frame(
            3,
            "session",
            "turn_finished",
            json!({"reason":"awaiting_input"}),
        ),
        frame(3, "assistant", "text", json!("worker")),
        frame(
            3,
            "tool",
            "use",
            json!({"call_id":"call","name":"tool","input":{}}),
        ),
        frame(
            3,
            "session",
            "model_request",
            json!({"model":"model","input_tokens":1}),
        ),
    ] {
        assert!(declined.clone().project(&later).is_err());
    }
    for turn in [None, Some("foreign".into())] {
        let mut foreign = mismatch();
        foreign.event.turn_id = turn;
        assert!(started().project(&foreign).is_err());
    }
    let mut foreign = mismatch();
    foreign.event.provenance = json!({"session_id":"other"});
    assert!(started().project(&foreign).is_err());
    for kind in ["turn_finished", "turn_failed", "turn_cancelled"] {
        let mut foreign = frame(
            3,
            "session",
            kind,
            if kind == "turn_cancelled" {
                Value::Null
            } else {
                json!({"reason":"done"})
            },
        );
        foreign.event.turn_id = Some("foreign".into());
        assert!(declined.clone().project(&foreign).is_err());
    }
    let mut late = started();
    late.project(&frame(2, "assistant", "text", json!("worker")))
        .unwrap();
    assert!(
        late.clone()
            .project(&frame(3, "session", "turn_started", Value::Null))
            .is_err()
    );
    assert!(late.project(&mismatch()).is_err());
    for activity in [
        frame(
            3,
            "plan",
            "updated",
            json!({"steps":[], "explanation":null}),
        ),
        frame(
            3,
            "approval",
            "requested",
            json!({"approval_id":"approval","kind":"shell"}),
        ),
    ] {
        assert!(declined.clone().project(&activity).is_err());
    }
}

#[test]
fn semantic_guard_cancellation_failure_and_discarded_projection_cannot_finish() {
    for kind in ["turn_cancelled", "turn_interrupted", "turn_failed"] {
        let mut projector = started();
        projector.project(&mismatch()).unwrap();
        let end = projector
            .project(&frame(
                3,
                "session",
                kind,
                if kind == "turn_failed" {
                    json!({"reason":"error"})
                } else {
                    Value::Null
                },
            ))
            .unwrap();
        assert!(matches!(
            end.effect,
            EventEffect::TurnEnded { semantic: None, .. }
        ));
    }
    let committed = started();
    committed.clone().project(&mismatch()).unwrap();
    let end = committed
        .clone()
        .project(&frame(
            3,
            "session",
            "turn_finished",
            json!({"reason":"done"}),
        ))
        .unwrap();
    assert!(matches!(
        end.effect,
        EventEffect::TurnEnded { semantic: None, .. }
    ));
}

#[test]
fn semantic_guard_wire_rejects_unknown_fields_and_unbounded_or_unsafe_text() {
    for value in [
        json!({"outcome":"compatible","reason":"unexpected"}),
        json!({"outcome":"mismatch","reason":"ok","extra":1}),
        json!({"outcome":"needs_clarification","reason":"ok"}),
        json!({"outcome":"unknown"}),
        json!({"outcome":"mismatch","reason":""}),
        json!({"outcome":"mismatch","reason":"line\nbreak"}),
        json!({"outcome":"mismatch","reason":"x".repeat(1025)}),
        json!({"outcome":"needs_clarification","reason":"ok","question":" "}),
    ] {
        assert!(started().project(&decision(value)).is_err());
    }
    assert!(
        SemanticGuardDecision::Mismatch {
            reason: "x".repeat(1024)
        }
        .validate()
        .is_ok()
    );
}
