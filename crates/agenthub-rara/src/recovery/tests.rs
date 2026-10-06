use serde_json::{Value, json};

use super::*;
use crate::{ControlRequest, EventEffect, EventFrame, EventProjector, RuntimeEvent};

fn state() -> Value {
    json!({"waiting_turn_id":null,"blocked":{"recovery_id":"token","turn_id":"old-turn","reason":"process_lost"},
        "decisions":[{"waiting_turn_id":"waiting","answer_turn_id":"answer","origin":{"runtime_id":"old-runtime","request_id":"request"},
            "answer_fingerprint":"a".repeat(64),"state":"uncertain"}],
        "last_resolution":{"recovery_id":"previous","note":"PRIVATE-NOTE"}})
}

fn request() -> Value {
    json!({"target":{"kind":"recovery","recovery_id":"token"},
        "guard":{"prompt":"Review work","context":{"role":"worker","card":"Rust","work":"Repair storage"}}})
}

#[test]
fn recovery_rejects_ambiguous_decision_records_and_bounded_private_data() {
    assert!(
        serde_json::from_value::<RecoveryStatus>(state())
            .unwrap()
            .validate()
            .is_ok()
    );
    for index in 0..8 {
        let mut value = state();
        match index {
            0 => value["decisions"][0]["answer_fingerprint"] = json!("invalid"),
            1 => value["decisions"][0]["origin"]["runtime_id"] = json!("old runtime"),
            2 => value["waiting_turn_id"] = json!("pending"),
            3 => value["decisions"][0]["answer_turn_id"] = json!("waiting"),
            4 => value["decisions"] = json!(vec![value["decisions"][0].clone(); 2]),
            5 => value["decisions"] = json!(vec![value["decisions"][0].clone(); 257]),
            6 => value["last_resolution"]["note"] = json!("note\ninjection"),
            _ => value["last_resolution"]["note"] = json!("x".repeat(4097)),
        }
        assert!(
            serde_json::from_value::<RecoveryStatus>(value)
                .unwrap()
                .validate()
                .is_err(),
            "case {index}"
        );
    }
    let mut extra = state();
    extra["decisions"][0]["answer"] = json!("must not be exposed");
    assert!(serde_json::from_value::<RecoveryStatus>(extra).is_err());
}

#[test]
fn recovery_controls_name_owned_session_without_admitting_a_turn() {
    for (request, method) in [
        (ControlRequest::QueryRecovery, "query_recovery"),
        (
            ControlRequest::ResolveRecovery(RecoveryResolution {
                recovery_id: "token".into(),
                note: "Owner retired; effects inspected".into(),
            }),
            "resolve_recovery",
        ),
        (
            ControlRequest::EvaluateReentry(serde_json::from_value(request()).unwrap()),
            "evaluate_reentry",
        ),
    ] {
        let frame = request
            .frame("runtime", "request", Some("session"))
            .unwrap();
        let value = serde_json::to_value(frame).unwrap();
        assert_eq!(
            value["payload"]["envelope"]["request"]["payload"]["type"],
            method
        );
        assert!(value["payload"]["expected_turn_id"].is_null());
        assert!(request.frame("runtime", "request", None).is_err());
    }
    for field in ["replay", "origin"] {
        let mut extra = request();
        extra[field] = json!(true);
        assert!(serde_json::from_value::<ReentryGuard>(extra).is_err());
    }
    let mut invalid: ReentryGuard = serde_json::from_value(request()).unwrap();
    invalid.guard.context.role.clear();
    assert!(
        ControlRequest::EvaluateReentry(invalid)
            .frame("runtime", "request", Some("session"))
            .is_err()
    );
}

fn event(kind: &str, payload: Value) -> EventFrame {
    EventFrame {
        runtime_id: "runtime".into(),
        session_id: "session".into(),
        event: RuntimeEvent {
            event_id: "event-1".into(),
            sequence: 1,
            turn_id: None,
            provenance: json!({"session_id":"session","controller":"runtime","trust":"trusted","authorship":"runtime"}),
            event: json!({"type":"session","payload":{"type":kind,"payload":payload}}),
        },
    }
}

#[test]
fn recovery_events_preserve_private_history_and_reject_foreign_authority() {
    for (kind, body) in [
        ("recovery_state", json!({"state":state()})),
        (
            "reentry_evaluated",
            json!({"evaluation":{"origin":{"runtime_id":"runtime","request_id":"entry"},
            "target":{"kind":"waiting","turn_id":"waiting"},
            "result":{"type":"decided","payload":{"decision":{"outcome":"mismatch","reason":"PRIVATE-NOTE"}}}}}),
        ),
    ] {
        let original = event(kind, body);
        let projected = EventProjector::new("runtime", "session")
            .unwrap()
            .project(&original)
            .unwrap();
        assert!(matches!(
            projected.effect,
            EventEffect::Recovery(_) | EventEffect::Reentry(_)
        ));
        assert!(!projected.safe_metadata.to_string().contains("PRIVATE-NOTE"));
        assert!(projected.history.iter().any(|entry| matches!(entry, crate::ProjectedHistory::Conversation(value) if value.to_string().contains("PRIVATE-NOTE"))));
        for change in 0..4 {
            let mut frame = original.clone();
            match change {
                0 => frame.event.turn_id = Some("worker-turn".into()),
                1 => frame.event.provenance["controller"] = json!("app_server"),
                2 => frame.event.provenance["trust"] = json!("untrusted"),
                _ => frame.event.provenance["session_id"] = json!("foreign"),
            }
            assert!(
                EventProjector::new("runtime", "session")
                    .unwrap()
                    .project(&frame)
                    .is_err()
            );
        }
    }
    let foreign = event(
        "reentry_evaluated",
        json!({"evaluation":{"origin":{"runtime_id":"retired","request_id":"entry"},
        "target":{"kind":"waiting","turn_id":"waiting"},"result":{"type":"unavailable","payload":{"reason":"timeout"}}}}),
    );
    assert!(
        EventProjector::new("runtime", "session")
            .unwrap()
            .project(&foreign)
            .is_err()
    );
}

#[test]
fn recovery_snapshot_cannot_also_claim_an_answerable_wait() {
    let mut frame = event(
        "runtime_state",
        json!({"snapshot":{"session_id":"session","phase":{"state":"recovery_required","detail":{"recovery_id":"token"}},
        "generation":0,"last_sequence":0,"pending_input":null}}),
    );
    assert!(matches!(
        EventProjector::new("runtime", "session")
            .unwrap()
            .project(&frame)
            .unwrap()
            .effect,
        EventEffect::Snapshot(crate::SessionSnapshot {
            phase: crate::SessionPhase::RecoveryRequired { .. },
            ..
        })
    ));
    frame.event.event["payload"]["payload"]["snapshot"]["pending_input"] = json!({"turn_id":"waiting","kind":{"type":"user","payload":{"question":"Which target?","options":[],"note":null}}});
    assert!(
        EventProjector::new("runtime", "session")
            .unwrap()
            .project(&frame)
            .is_err()
    );
}
