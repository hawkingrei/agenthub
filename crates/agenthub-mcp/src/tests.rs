use serde_json::json;
use tokio::io::BufReader;

use crate::{
    MAX_MESSAGE_BYTES, McpTransportError,
    protocol::{MessageKind, ProtocolVersion, message_kind},
    sse::SseDecoder,
    stdio::{read_message, write_message},
};

#[tokio::test]
async fn jsonl_round_trips_requests_notifications_results_and_extensions() {
    let messages = [
        json!({"jsonrpc":"2.0","id":"request","method":"tools/call","params":{"name":"nested/tool","arguments":{"text":"line1\nline2"}},"vendor":{"extra":true}}),
        json!({"jsonrpc":"2.0","method":"notifications/progress","params":{"progress":1}}),
        json!({"jsonrpc":"2.0","id":"request","result":{"structuredContent":{"nested":[true,null,42]},"content":[{"type":"text","text":"raw-result"}]}}),
    ];
    let mut wire = Vec::new();
    for message in &messages {
        write_message(&mut wire, message).await.unwrap();
    }
    assert_eq!(
        wire.iter().filter(|byte| **byte == b'\n').count(),
        messages.len()
    );
    let mut input = BufReader::with_capacity(1, wire.as_slice());
    for message in &messages {
        assert_eq!(
            read_message(&mut input).await.unwrap().as_ref(),
            Some(message)
        );
    }
    assert!(read_message(&mut input).await.unwrap().is_none());
}

#[tokio::test]
async fn jsonl_rejects_unframed_invalid_and_unbounded_input_without_echoing_it() {
    for bytes in [
        b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}".as_slice(),
        b"secret-not-json\n",
        b"\n",
    ] {
        let error = read_message(&mut BufReader::new(bytes))
            .await
            .err()
            .unwrap();
        assert_eq!(error, McpTransportError::InvalidMessage);
        assert!(!error.to_string().contains("secret-not-json"));
    }
    let bytes = vec![b'x'; MAX_MESSAGE_BYTES + 3];
    let mut input = BufReader::with_capacity(1024, bytes.as_slice());
    assert_eq!(
        read_message(&mut input).await.err(),
        Some(McpTransportError::MessageTooLarge)
    );
}

#[test]
fn rpc_envelope_validation_preserves_real_error_data_and_rejects_mixed_shapes() {
    assert_eq!(message_kind(&json!({"jsonrpc":"2.0","error":{"code":-32022,"message":"unsupported","data":{"supported":[]}}})).unwrap(),MessageKind::Response);
    for message in [
        json!({"jsonrpc":"2.0","id":1,"method":"ping","result":{}}),
        json!({"jsonrpc":"2.0","id":1,"result":{},"error":{"code":1,"message":"bad"}}),
        json!({"jsonrpc":"2.0","id":null,"method":"ping"}),
        json!({"jsonrpc":"2.0","id":1.5,"method":"ping"}),
        json!({"jsonrpc":"2.0","id":1,"error":{"code":"wrong","message":"bad"}}),
        json!({"jsonrpc":"2.0","id":1}),
    ] {
        assert!(message_kind(&message).is_err());
    }
    assert!("2099-01-01".parse::<ProtocolVersion>().is_err());
    assert!(
        "2025-11-25"
            .parse::<ProtocolVersion>()
            .unwrap()
            .uses_initialization()
    );
    assert!(
        !"2026-07-28"
            .parse::<ProtocolVersion>()
            .unwrap()
            .uses_initialization()
    );
}

#[test]
fn sse_handles_every_chunk_boundary_bom_comments_and_line_endings() {
    let source = b"\xef\xbb\xbf: keepalive\r\nid: first\rdata: {\"jsonrpc\":\"2.0\",\r\ndata: \"id\":1,\"result\":{}}\n\nretry: 10\nid: second\ndata:\n\n";
    for split in 0..=source.len() {
        let mut decoder = SseDecoder::new();
        let mut frames = decoder.push(&source[..split]).unwrap();
        frames.extend(decoder.push(&source[split..]).unwrap());
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].cursor.as_deref(), Some("first"));
        assert_eq!(
            crate::protocol::parse_message(&frames[0].data).unwrap()["id"],
            1
        );
        assert_eq!(frames[1].retry_ms, Some(10));
        assert!(frames[1].data.is_empty());
    }
}

#[test]
fn sse_does_not_dispatch_partial_frames_and_bounds_undelimited_data() {
    let mut decoder = SseDecoder::new();
    assert!(
        decoder
            .push(b"data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n")
            .unwrap()
            .is_empty()
    );
    assert_eq!(decoder.push(b"\n").unwrap().len(), 1);
    assert!(
        decoder
            .push(b"id: nul\0invalid\nretry: invalid\n\n")
            .unwrap()
            .is_empty()
    );
    let mut decoder = SseDecoder::new();
    assert!(matches!(
        decoder.push(&vec![b'x'; MAX_MESSAGE_BYTES + 1]),
        Err(McpTransportError::MessageTooLarge)
    ));
}
