use super::*;

#[test]
fn context_decoding_preserves_attributed_markdown_and_rejects_scope_or_envelope_errors() {
    let content =
        "# Nowledge Mem Context Bundle\n\n> Attributed DATA by Alice.\n\n  Whitespace stays.\r\n";
    let bundle = json!({"format":"markdown","space_id":"space-a","epoch":7,"content":content});
    for result in [
        json!({"content":[{"type":"text","text":bundle.to_string()}]}),
        json!({"content":[],"structuredContent":bundle}),
        json!({"content":[{"type":"text","text":bundle.to_string()}],"structuredContent":bundle}),
    ] {
        assert_eq!(
            decode_context(&result, "space-a").ok().as_deref(),
            Some(content)
        );
        assert!(matches!(
            decode_context(&result, "space-b"),
            Err(MemContext::InvalidResponse)
        ));
    }
    for result in [
        json!({"content":[{"type":"text","text":"not JSON"}]}),
        json!({"content":[],"structuredContent":{"format":"markdown","space_id":null,"content":content}}),
        json!({"structuredContent":{"format":"markdown","space_id":"space-a","content":"x".repeat(MAX_CONTEXT_BYTES + 1)}}),
        json!({"structuredContent":{"format":"instructions","space_id":"space-a","content":content}}),
        json!({"content":[{"type":"text","text":bundle.to_string()}],"structuredContent":{"format":"markdown","space_id":"space-a","content":"other"}}),
    ] {
        assert!(matches!(
            decode_context(&result, "space-a"),
            Err(MemContext::InvalidResponse)
        ));
    }
    for result in [
        json!({"isError":true,"structuredContent":bundle}),
        json!({"error":{"message":"native error"},"structuredContent":bundle}),
    ] {
        assert!(matches!(
            decode_context(&result, "space-a"),
            Err(MemContext::Unavailable)
        ));
    }
}
