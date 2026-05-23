use serde_json::{Map, Value};

use super::mailbox::ResolvedChannelMailboxTarget;

pub(super) fn build_channel_mailbox_forward_payload(
    source_payload: &Value,
    target: &ResolvedChannelMailboxTarget,
    channel_id: &str,
    authority_message_id: i64,
    mention_actor_ids: &[String],
) -> Value {
    let mut payload_obj = match source_payload {
        Value::Object(map) => map.clone(),
        _ => Map::new(),
    };
    let mentions = mention_actor_ids
        .iter()
        .cloned()
        .map(Value::String)
        .collect::<Vec<_>>();
    payload_obj.insert(
        "channel_id".to_string(),
        Value::String(channel_id.to_string()),
    );
    payload_obj.insert("team_id".to_string(), Value::String(target.team_id.clone()));
    payload_obj.insert(
        "channel_conversation_id".to_string(),
        Value::String(target.conversation_id.clone()),
    );
    payload_obj.insert("task_id".to_string(), Value::String(target.task_id.clone()));
    payload_obj.insert(
        "authority_message_id".to_string(),
        Value::Number(authority_message_id.into()),
    );
    payload_obj.insert(
        "delivery_scope".to_string(),
        Value::String("channel_broadcast".to_string()),
    );
    payload_obj.insert(
        "mention_actor_ids".to_string(),
        Value::Array(mentions.clone()),
    );
    payload_obj.insert("mentioned_actor_ids".to_string(), Value::Array(mentions));
    Value::Object(payload_obj)
}

fn is_valid_mention_char(raw: u8) -> bool {
    matches!(raw, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_')
}

fn is_email_local_char(raw: u8) -> bool {
    matches!(
        raw,
        b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'_' | b'%' | b'+' | b'-'
    )
}

pub(super) fn find_raw_actor_mention_range(text: &str, actor_id: &str) -> Option<(usize, usize)> {
    let needle = format!("@{actor_id}");
    let bytes = text.as_bytes();
    let mut cursor = 0usize;
    while let Some(found) = text[cursor..].find(needle.as_str()) {
        let start = cursor + found;
        let end = start + needle.len();
        let left_ok = start == 0 || !is_email_local_char(bytes[start - 1]);
        let right_ok = end >= bytes.len() || !is_valid_mention_char(bytes[end]);
        if left_ok && right_ok {
            return Some((start, end));
        }
        cursor = end;
    }
    None
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::team::manager::mailbox::ResolvedChannelMailboxTarget;
    use crate::team::manager::mailbox_payloads::channel_payload_correlation_id;

    #[test]
    fn build_channel_mailbox_forward_payload_carries_metadata_and_mentions() {
        let payload = build_channel_mailbox_forward_payload(
            &json!({
                "type": "chat_message",
                "text": "@reviewer please inspect this",
                "correlation_id": "corr-1"
            }),
            &ResolvedChannelMailboxTarget {
                team_id: "team-1".to_string(),
                task_id: "task-1".to_string(),
                conversation_id: "conversation-1".to_string(),
                recipient_actor_ids: vec!["reviewer".to_string()],
            },
            "all",
            42,
            &["reviewer".to_string(), "worker-b".to_string()],
        );

        assert_eq!(
            payload.get("delivery_scope"),
            Some(&Value::String("channel_broadcast".to_string()))
        );
        assert_eq!(
            payload.get("team_id"),
            Some(&Value::String("team-1".to_string()))
        );
        assert_eq!(
            payload.get("channel_conversation_id"),
            Some(&Value::String("conversation-1".to_string()))
        );
        assert_eq!(
            payload.get("task_id"),
            Some(&Value::String("task-1".to_string()))
        );
        assert_eq!(payload.get("authority_message_id"), Some(&json!(42)));
        assert_eq!(
            payload.get("mentioned_actor_ids"),
            Some(&json!(["reviewer", "worker-b"]))
        );
        assert_eq!(channel_payload_correlation_id(&payload), Some("corr-1"));
    }
}
