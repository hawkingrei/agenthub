use super::*;
use crate::{ControlRequest, SourceRegistration};

fn source(id: &str) -> McpSource {
    McpSource {
        source_id: id.into(),
        command: "/usr/bin/agenthub".into(),
        args: vec!["mcp-proxy".into(), "--server-id".into(), id.into()],
        env: BTreeMap::from([(
            "AGENTHUB_LOOP_CREDENTIAL_FILE".into(),
            "/private/activation.json".into(),
        )]),
    }
}

fn handshake() -> Handshake {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../fixtures/stdio-v1.json")).unwrap();
    let handshake: Handshake =
        serde_json::from_value(fixture["frames"][0]["payload"].clone()).unwrap();
    handshake.validate().unwrap();
    handshake
}

#[test]
fn mcp_sources_require_all_controls_and_events_before_registration() {
    let hello = handshake();
    let source = SourceRegistration::Mcp(source("app-a"));
    source.require_capability(&hello).unwrap();
    for missing in [
        "mcp_source.register",
        "mcp_source.unregister",
        "mcp_source.query",
        "mcp",
    ] {
        let mut changed = hello.clone();
        changed.request_methods.retain(|method| method != missing);
        changed.event_families.retain(|family| family != missing);
        changed.validate().unwrap();
        assert_eq!(
            source.require_capability(&changed),
            Err(ProtocolError::UnsupportedHandshake)
        );
    }
    let request = ControlRequest::RegisterSource(source);
    assert!(request.frame("runtime", "register", None).is_err());
    let wire = serde_json::to_value(
        request
            .frame("runtime", "register", Some("session"))
            .unwrap(),
    )
    .unwrap();
    let envelope = &wire["payload"]["envelope"];
    assert_eq!(envelope["request"]["type"], "mcp_source");
    assert_eq!(envelope["request"]["payload"]["type"], "register");
    assert_eq!(envelope["provenance"]["session_id"], "session");
    assert_eq!(envelope["provenance"]["source_id"], "app-a");
    assert_eq!(
        envelope["request"]["payload"]["payload"]["env"],
        serde_json::json!({"AGENTHUB_LOOP_CREDENTIAL_FILE":"/private/activation.json"})
    );
    assert!(wire["payload"].get("expected_turn_id").is_none());
}

#[test]
fn mcp_launch_limits_and_duplicates_fail_before_any_source_is_sent() {
    let hello = handshake();
    let mut batch: Vec<_> = (0..16)
        .map(|i| SourceRegistration::Mcp(source(&format!("source-{i}"))))
        .collect();
    SourceRegistration::validate_batch(&batch, &hello).unwrap();
    batch.push(SourceRegistration::Mcp(source("overflow")));
    assert_eq!(
        SourceRegistration::validate_batch(&batch, &hello),
        Err(ProtocolError::FrameTooLarge)
    );
    assert_eq!(
        SourceRegistration::validate_batch(&[batch[0].clone(), batch[0].clone()], &hello),
        Err(ProtocolError::InvalidIdentity)
    );
    let mut invalid = Vec::new();
    let mut changed = source("app-a");
    changed.command = "relative".into();
    invalid.push(changed);
    let mut changed = source("app-a");
    changed.args.push("nul\0argument".into());
    invalid.push(changed);
    let mut changed = source("app-a");
    changed.env.insert("BAD=KEY".into(), "value".into());
    invalid.push(changed);
    let mut changed = source("app-a");
    changed.env.insert("KEY".into(), "x".repeat(64 * 1024));
    invalid.push(changed);
    for source in invalid {
        assert!(
            SourceRegistration::validate_batch(&[SourceRegistration::Mcp(source)], &hello).is_err()
        );
    }
}
