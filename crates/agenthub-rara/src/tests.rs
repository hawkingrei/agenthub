use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncWriteExt, BufReader};

use crate::*;

fn fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/stdio-v1.json")).unwrap()
}

fn hello() -> Handshake {
    serde_json::from_value(fixture()["frames"][0]["payload"].clone()).unwrap()
}

async fn decode(value: Value) -> Result<ServerFrame, ProtocolError> {
    let mut encoded = serde_json::to_vec(&value).unwrap();
    encoded.push(b'\n');
    FrameReader::new(encoded.as_slice()).next().await
}

fn prompt() -> ClientFrame {
    ClientFrame::Control {
        runtime_id: "runtime-fixture".into(),
        envelope: ControlEnvelope {
            request_id: "prompt".into(),
            provenance: Provenance::new(Some("session-fixture".into())),
            request: json!({"type":"input","payload":{"type":"submit_user_prompt","payload":{"prompt":""}}}),
        },
        expected_turn_id: None,
    }
}

#[tokio::test]
async fn pinned_real_process_frames_preserve_event_session_and_original_identity() {
    let fixture = fixture();
    assert_eq!(fixture["upstream_commit"], PINNED_UPSTREAM_REVISION);
    for value in fixture["frames"].as_array().unwrap() {
        let frame = decode(value.clone()).await.expect("pinned frame");
        assert_eq!(serde_json::to_value(&frame).unwrap(), *value);
        assert_eq!(frame.runtime_id(), "runtime-fixture");
        if let ServerFrame::Event(frame) = frame {
            assert_eq!(frame.session_id, "session-fixture");
            assert_eq!(frame.event.event_id, "ctl-0000000000000001");
            assert_eq!(frame.event.sequence, 1);
            assert!(frame.event.provenance["session_id"].is_null());
        }
    }
}

#[test]
fn handshake_checks_methods_and_lifetimes_instead_of_version_label_alone() {
    let hello = hello();
    hello.validate().unwrap();
    assert!(!hello.supports("session.resume"));
    assert!(!hello.capabilities.approval_persistence);
    assert_eq!(
        hello.require_methods(&["session.resume"]),
        Err(ProtocolError::UnsupportedHandshake)
    );
    let mut missing = hello.clone();
    missing
        .request_methods
        .retain(|method| method != "input.answer_shell");
    assert_eq!(missing.validate(), Err(ProtocolError::UnsupportedHandshake));
    for version in [0, 2, u32::MAX] {
        let mut changed = hello.clone();
        changed.protocol_version = version;
        assert_eq!(changed.validate(), Err(ProtocolError::UnsupportedHandshake));
    }
    let mut changed = hello;
    changed.transport = "acp".into();
    assert_eq!(changed.validate(), Err(ProtocolError::UnsupportedHandshake));
}

#[test]
fn capabilities_reject_inconsistent_lists_and_empty_resource_bounds() {
    let mut cases = Vec::new();
    let mut changed = hello();
    changed
        .request_methods
        .push(changed.request_methods[0].clone());
    cases.push(changed);
    let mut changed = hello();
    changed.request_families.push("fake".into());
    cases.push(changed);
    let mut changed = hello();
    changed.event_families = (0..65).map(|i| format!("family-{i}")).collect();
    cases.push(changed);
    let mut changed = hello();
    changed.capabilities.request_receipts = ReceiptCapability::Runtime { max_requests: 0 };
    cases.push(changed);
    let mut changed = hello();
    changed.capabilities.replay = ReplayCapability::Unavailable;
    cases.push(changed);
    for changed in cases {
        assert_eq!(changed.validate(), Err(ProtocolError::InvalidCapabilities));
    }

    let mut no_replay = hello();
    no_replay.capabilities.replay = ReplayCapability::Unavailable;
    no_replay.request_methods.retain(|m| m != "output.replay");
    no_replay.request_families.retain(|f| f != "output");
    no_replay.validate().unwrap();
    assert!(!no_replay.supports("output.replay"));
}

#[tokio::test]
async fn malformed_identity_and_handshake_errors_never_echo_private_input() {
    for field in ["runtime_id", "runtime_version", "provider", "model"] {
        let mut frame = fixture()["frames"][0].clone();
        frame["payload"][field] = json!("private-provider-key\n");
        let error = decode(frame).await.unwrap_err();
        assert_eq!(error, ProtocolError::InvalidIdentity);
        assert!(!error.to_string().contains("private-provider-key"));
    }
    let mut unknown = fixture()["frames"][0].clone();
    unknown["payload"]["raw_credentials"] = json!("private-provider-key");
    assert_eq!(decode(unknown).await, Err(ProtocolError::MalformedFrame));
    let mut event = fixture()["frames"][2].clone();
    event["payload"]
        .as_object_mut()
        .unwrap()
        .remove("session_id");
    assert_eq!(decode(event).await, Err(ProtocolError::MalformedFrame));
}

#[tokio::test]
async fn cancelled_read_preserves_a_partial_frame() {
    let encoded = serde_json::to_vec(&fixture()["frames"][0]).unwrap();
    let split = encoded.len() / 2;
    let (input, mut peer) = tokio::io::duplex(encoded.len() + 1);
    let mut reader = FrameReader::new(BufReader::new(input));
    peer.write_all(&encoded[..split]).await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(20), reader.next())
            .await
            .is_err()
    );
    peer.write_all(&encoded[split..]).await.unwrap();
    peer.write_all(b"\n").await.unwrap();
    assert!(matches!(reader.next().await, Ok(ServerFrame::Handshake(_))));
}

#[tokio::test]
async fn framing_rejects_truncated_blank_invalid_and_oversized_payloads() {
    for bytes in [
        b"\n".as_slice(),
        b"\r\n",
        b"not-json\n",
        b"{\"type\":",
        b"\xff\n",
    ] {
        assert_eq!(
            FrameReader::new(bytes).next().await,
            Err(ProtocolError::MalformedFrame)
        );
    }
    assert_eq!(
        FrameReader::new(b"".as_slice()).next().await,
        Err(ProtocolError::TransportLost)
    );
    let mut encoded = serde_json::to_vec(&fixture()["frames"][1]).unwrap();
    encoded.resize(MAX_FRAME_BYTES, b' ');
    encoded.extend_from_slice(b"\r\n");
    assert!(matches!(
        FrameReader::new(encoded.as_slice()).next().await,
        Ok(ServerFrame::Ack(_))
    ));
    let oversized = vec![b'x'; MAX_FRAME_BYTES + 2];
    assert_eq!(
        FrameReader::new(oversized.as_slice()).next().await,
        Err(ProtocolError::FrameTooLarge)
    );
}

#[test]
fn outbound_target_validation_and_byte_bound_precede_any_write() {
    let mut request = prompt();
    let initial_size = encode_request(&request).unwrap().len() - 1;
    if let ClientFrame::Control { envelope, .. } = &mut request {
        envelope.request["payload"]["payload"]["prompt"] =
            json!("x".repeat(MAX_FRAME_BYTES - initial_size));
    }
    assert_eq!(encode_request(&request).unwrap().len(), MAX_FRAME_BYTES + 1);
    if let ClientFrame::Control { envelope, .. } = &mut request {
        envelope.request["payload"]["payload"]["prompt"] = json!("x".repeat(MAX_FRAME_BYTES));
    }
    assert_eq!(encode_request(&request), Err(ProtocolError::FrameTooLarge));
    let mut request = prompt();
    if let ClientFrame::Control { envelope, .. } = &mut request {
        envelope.provenance.session_id = None;
    }
    assert_eq!(encode_request(&request), Err(ProtocolError::InvalidTarget));
    let mut request = prompt();
    if let ClientFrame::Control {
        envelope,
        expected_turn_id,
        ..
    } = &mut request
    {
        envelope.request = json!({"type":"session","payload":{"type":"cancel_current_turn"}});
        *expected_turn_id = None;
    }
    assert_eq!(encode_request(&request), Err(ProtocolError::InvalidTarget));
    if let ClientFrame::Control {
        expected_turn_id, ..
    } = &mut request
    {
        *expected_turn_id = Some("turn-1".into());
    }
    let wire: Value = serde_json::from_slice(&encode_request(&request).unwrap()).unwrap();
    assert_eq!(wire["payload"]["expected_turn_id"], "turn-1");
    assert_eq!(
        wire["payload"]["envelope"]["provenance"]["trust"],
        "untrusted"
    );
}

#[tokio::test]
async fn replay_gap_and_acknowledgement_shapes_remain_explicit() {
    for result in [
        json!({"status":"queued","session_id":"s"}),
        json!({"status":"rejected","code":"busy","message":"session is busy"}),
    ] {
        let value =
            json!({"type":"ack","payload":{"runtime_id":"r","request_id":"q","result":result}});
        assert!(matches!(decode(value).await, Ok(ServerFrame::Ack(_))));
    }
    let gap = json!({"type":"replay_gap","payload":{"runtime_id":"r","request_id":"q","session_id":"s","requested_after":2,"oldest_available":5,"latest":8}});
    assert!(matches!(
        decode(gap.clone()).await,
        Ok(ServerFrame::ReplayGap(_))
    ));
    let mut invalid = gap;
    invalid["payload"]["requested_after"] = json!(6);
    assert_eq!(decode(invalid).await, Err(ProtocolError::MalformedFrame));
}
