use std::collections::BTreeSet;

use serde_json::{Map, Value};
use uuid::Uuid;

use super::mailbox::{
    TEAM_SPECIAL_USER_ACTOR_ALIAS, TEAM_SPECIAL_USER_ACTOR_PREFIX, find_raw_actor_mention_range,
};
use agenthub_team_actor::ACTOR_MAIN_PEER_ID;

#[derive(Debug, Clone)]
pub(super) struct CanonicalChatReply {
    pub(super) text: String,
    pub(super) correlation_id: Option<String>,
}

pub(super) fn should_persist_human_visible_chat_reply_for_payload(
    transport: &agenthub_team_actor::ActorMessageTransport,
    to_actor_id: &str,
    to_peer_id: &str,
    from_actor_id: &str,
    payload: &Value,
) -> bool {
    *transport == agenthub_team_actor::ActorMessageTransport::Local
        && to_peer_id == ACTOR_MAIN_PEER_ID
        && is_human_actor_id(to_actor_id)
        && !is_human_actor_id(from_actor_id)
        && resolve_canonical_chat_reply(payload).is_some()
}

pub(super) fn is_human_actor_id(actor_id: &str) -> bool {
    let trimmed = actor_id.trim();
    if trimmed == TEAM_SPECIAL_USER_ACTOR_ALIAS {
        return true;
    }
    if let Some(suffix) = trimmed.strip_prefix(TEAM_SPECIAL_USER_ACTOR_PREFIX) {
        return !suffix.trim().is_empty();
    }
    false
}

pub(super) fn normalize_channel_message_payload(payload: Value) -> Value {
    let payload_obj = match payload {
        Value::Object(map) => map,
        Value::String(text) => {
            let mut payload_obj = Map::new();
            payload_obj.insert(
                "type".to_string(),
                Value::String("chat_message".to_string()),
            );
            payload_obj.insert("text".to_string(), Value::String(text));
            payload_obj
        }
        other => {
            let mut payload_obj = Map::new();
            payload_obj.insert(
                "type".to_string(),
                Value::String("chat_message".to_string()),
            );
            payload_obj.insert("text".to_string(), Value::String(other.to_string()));
            payload_obj
        }
    };
    Value::Object(payload_obj)
}

pub(super) fn ensure_channel_message_correlation_id(
    payload: Value,
    fallback_correlation_id: Option<&str>,
) -> Value {
    let mut payload_obj = match normalize_channel_message_payload(payload) {
        Value::Object(map) => map,
        _ => unreachable!("channel payload normalization should always yield an object"),
    };
    let has_correlation_id = payload_obj
        .get("correlation_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    if !has_correlation_id {
        payload_obj.insert(
            "correlation_id".to_string(),
            Value::String(
                fallback_correlation_id
                    .map(str::to_string)
                    .unwrap_or_else(|| Uuid::new_v4().to_string()),
            ),
        );
    }
    Value::Object(payload_obj)
}

pub(super) fn channel_payload_correlation_id(payload: &Value) -> Option<&str> {
    payload
        .get("correlation_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(super) fn collect_channel_mention_actor_ids(
    payload: &Value,
    member_ids: &BTreeSet<String>,
) -> Vec<String> {
    let mut mentioned = BTreeSet::new();
    for key in ["mention_actor_ids", "mentioned_actor_ids"] {
        if let Some(explicit_mentions) = payload.get(key).and_then(Value::as_array) {
            for mention in explicit_mentions {
                if let Some(actor_id) = mention.as_str().map(str::trim)
                    && member_ids.contains(actor_id)
                {
                    mentioned.insert(actor_id.to_string());
                }
            }
        }
    }

    let text = match payload {
        Value::String(text) => Some(text.as_str()),
        Value::Object(map) => map.get("text").and_then(Value::as_str),
        _ => None,
    };
    if let Some(text) = text {
        for actor_id in member_ids {
            if find_raw_actor_mention_range(text, actor_id).is_some() {
                mentioned.insert(actor_id.clone());
            }
        }
    }

    mentioned.into_iter().collect()
}

pub(super) fn resolve_canonical_chat_reply(payload: &Value) -> Option<CanonicalChatReply> {
    match payload {
        Value::Object(map) => resolve_canonical_chat_reply_from_map(map),
        Value::String(text) => {
            if let Some(parsed) = parse_stringified_json_payload(text)
                && let Some(reply) = resolve_canonical_chat_reply(&parsed)
            {
                return Some(reply);
            }
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(CanonicalChatReply {
                    text: text.clone(),
                    correlation_id: None,
                })
            }
        }
        _ => None,
    }
}

pub(super) fn build_canonical_chat_payload(reply: &CanonicalChatReply) -> Value {
    let mut payload = Map::new();
    payload.insert(
        "type".to_string(),
        Value::String("chat_message".to_string()),
    );
    payload.insert("text".to_string(), Value::String(reply.text.clone()));
    if let Some(correlation_id) = reply.correlation_id.as_deref() {
        payload.insert(
            "correlation_id".to_string(),
            Value::String(correlation_id.to_string()),
        );
    }
    Value::Object(payload)
}

fn resolve_canonical_chat_reply_from_map(map: &Map<String, Value>) -> Option<CanonicalChatReply> {
    let payload_type = map
        .get("type")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if !payload_type.is_empty() && payload_type != "chat_message" {
        return None;
    }
    let text = map
        .get("text")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let correlation_id = map
        .get("correlation_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    Some(CanonicalChatReply {
        text,
        correlation_id,
    })
}

pub(super) fn parse_stringified_json_payload(raw: &str) -> Option<Value> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}
