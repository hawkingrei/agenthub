use serde_json::Value;

use super::mailbox_payloads::{
    CanonicalChatReply, parse_stringified_json_payload, resolve_canonical_chat_reply,
};
use super::mailbox_reply_obligation_snapshots::ReplyObligationMessageSnapshot;
use crate::team::TeamActorMessageRecord;

pub(super) fn mailbox_resolution_kind(payload: &Value) -> Option<&str> {
    payload
        .as_object()
        .and_then(|map| map.get("mailbox_resolution"))
        .and_then(Value::as_object)
        .and_then(|map| map.get("kind"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(super) fn mailbox_resolution_reason(payload: &Value) -> Option<&str> {
    payload
        .as_object()
        .and_then(|map| map.get("mailbox_resolution"))
        .and_then(Value::as_object)
        .and_then(|map| map.get("reason"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(super) fn resolve_canonical_chat_reply_from_snapshot(
    message: &ReplyObligationMessageSnapshot,
) -> Option<CanonicalChatReply> {
    if let Some(raw_text) = message.reply_string_payload.as_deref() {
        if let Some(parsed) = parse_stringified_json_payload(raw_text)
            && let Some(reply) = resolve_canonical_chat_reply(&parsed)
        {
            return Some(reply);
        }
        let trimmed = raw_text.trim();
        if trimmed.is_empty() {
            return None;
        }
        return Some(CanonicalChatReply {
            text: raw_text.to_string(),
            correlation_id: None,
        });
    }
    let payload_type = message
        .reply_payload_type
        .as_deref()
        .map(str::trim)
        .unwrap_or_default();
    if !payload_type.is_empty() && payload_type != "chat_message" {
        return None;
    }
    let text = message
        .reply_text
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    Some(CanonicalChatReply {
        text,
        correlation_id: message.reply_correlation_id.clone(),
    })
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn reply_obligation_snapshot_from_message(
    message: &TeamActorMessageRecord,
) -> ReplyObligationMessageSnapshot {
    let envelope = message.inbound_envelope();
    let reply = resolve_canonical_chat_reply(&message.payload);
    ReplyObligationMessageSnapshot {
        message_id: message.message_id,
        from_actor_id: message.from_actor_id.clone(),
        to_actor_id: message.to_actor_id.clone(),
        to_peer_id: message.to_peer_id.clone(),
        transport: message.transport.clone(),
        status: message.status.clone(),
        handling_disposition: message.handling_disposition.clone(),
        message_kind: message.message_kind.clone(),
        source_surface: Some(envelope.source_surface),
        reply_target: envelope.reply_target,
        conversation_id: envelope.conversation_id,
        thread_root_message_id: envelope.thread_root_message_id,
        requires_user_visible_reply: envelope.requires_user_visible_reply,
        mailbox_resolution_kind: mailbox_resolution_kind(&message.payload).map(str::to_string),
        mailbox_resolution_reason: mailbox_resolution_reason(&message.payload).map(str::to_string),
        reply_payload_type: message
            .payload
            .get("type")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        reply_text: reply.as_ref().map(|reply| reply.text.clone()),
        reply_correlation_id: reply
            .as_ref()
            .and_then(|reply| reply.correlation_id.clone()),
        reply_string_payload: match &message.payload {
            Value::String(text) => Some(text.clone()),
            _ => None,
        },
        created_at: message.created_at,
    }
}
