use super::memory_flush_event_loader::MemoryFlushEventRow;
use super::{MEMORY_FLUSH_MAX_EXCERPT_CHARS, MEMORY_FLUSH_MAX_SUMMARY_CHARS};
use crate::agent::event_message_codec::decode_message_from_storage;
use agenthub_text::truncate_chars;
use serde_json::Value;

pub(super) fn build_memory_flush_observation(row: &MemoryFlushEventRow) -> Value {
    let event_id = row.id;
    let stream = row.stream.as_str();
    let message = decode_message_from_storage(row.message.as_slice());
    if let Ok(message_json) = serde_json::from_str::<Value>(&message) {
        let redacted = super::redact_sensitive_json(&message_json);
        let observation_type = message_json
            .as_object()
            .and_then(|obj| obj.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("json_message");
        let excerpt = truncate_chars(
            redacted.to_string().as_str(),
            MEMORY_FLUSH_MAX_EXCERPT_CHARS,
        );
        return serde_json::json!({
            "event_id": event_id,
            "stream": stream,
            "type": observation_type,
            "excerpt": excerpt,
        });
    }

    serde_json::json!({
        "event_id": event_id,
        "stream": stream,
        "type": "text_message",
        "excerpt": truncate_chars(message.as_str(), MEMORY_FLUSH_MAX_EXCERPT_CHARS),
    })
}

pub(super) fn build_memory_flush_summary(observations: &[Value]) -> String {
    let mut lines = Vec::new();
    for observation in observations.iter().take(5) {
        let event_id = observation
            .get("event_id")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let observation_type = observation
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let excerpt = observation
            .get("excerpt")
            .and_then(Value::as_str)
            .unwrap_or_default();
        lines.push(format!("#{event_id} [{observation_type}] {excerpt}"));
    }
    truncate_chars(lines.join("\n").as_str(), MEMORY_FLUSH_MAX_SUMMARY_CHARS)
}
