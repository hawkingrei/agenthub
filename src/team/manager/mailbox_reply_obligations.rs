use serde_json::{Map, Value};

use super::mailbox::{
    MAILBOX_RESOLUTION_ESCALATED, MAILBOX_RESOLUTION_IGNORED, MAILBOX_RESOLUTION_TAKEN_OVER,
    MAILBOX_RESOLUTION_TRANSFERRED, ReplyActorPairKey,
};
use super::mailbox_reply_obligation_snapshot_conversion::{
    mailbox_resolution_kind, mailbox_resolution_reason,
};
use crate::team::TeamActorMessageRecord;
use agenthub_team_actor::{ActorIdentityKind, ActorMessageHandlingDisposition};

pub(super) fn reply_actor_pair_for_inbound_obligation(
    message: &TeamActorMessageRecord,
) -> Option<ReplyActorPairKey> {
    let envelope = message.inbound_envelope();
    if !envelope.requires_user_visible_reply {
        return None;
    }
    if message.from_actor_kind != ActorIdentityKind::Human
        || message.to_actor_kind != ActorIdentityKind::Agent
    {
        return None;
    }
    Some(ReplyActorPairKey {
        agent_actor_id: message.to_actor_id.clone(),
        human_actor_id: message.from_actor_id.clone(),
    })
}

pub(super) fn reply_obligation_is_terminal(message: &TeamActorMessageRecord) -> bool {
    if message.handling_disposition == ActorMessageHandlingDisposition::Ignored {
        return mailbox_resolution_kind(&message.payload) == Some(MAILBOX_RESOLUTION_IGNORED)
            && mailbox_resolution_reason(&message.payload).is_some();
    }
    matches!(
        message.handling_disposition,
        ActorMessageHandlingDisposition::Released
    ) && matches!(
        mailbox_resolution_kind(&message.payload),
        Some(
            MAILBOX_RESOLUTION_ESCALATED
                | MAILBOX_RESOLUTION_TRANSFERRED
                | MAILBOX_RESOLUTION_TAKEN_OVER,
        )
    )
}

pub(super) fn build_mailbox_resolution_payload(
    source_payload: &Value,
    kind: &str,
    resolved_by_actor_id: &str,
    target_actor_id: &str,
    resolved_at: i64,
) -> Value {
    let mut payload_obj = match source_payload {
        Value::Object(map) => map.clone(),
        _ => Map::new(),
    };
    payload_obj.insert(
        "mailbox_resolution".to_string(),
        serde_json::json!({
            "kind": kind,
            "resolved_by_actor_id": resolved_by_actor_id,
            "target_actor_id": target_actor_id,
            "resolved_at": resolved_at
        }),
    );
    Value::Object(payload_obj)
}

pub(super) fn build_ignored_mailbox_payload(
    source_payload: &Value,
    ignored_by_actor_id: &str,
    reason: &str,
    ignored_at: i64,
) -> Value {
    let mut payload_obj = match source_payload {
        Value::Object(map) => map.clone(),
        _ => Map::new(),
    };
    payload_obj.insert(
        "mailbox_resolution".to_string(),
        serde_json::json!({
            "kind": MAILBOX_RESOLUTION_IGNORED,
            "resolved_by_actor_id": ignored_by_actor_id,
            "reason": reason,
            "resolved_at": ignored_at
        }),
    );
    Value::Object(payload_obj)
}

pub(super) fn build_escalated_mailbox_payload(
    source_payload: &Value,
    source_message_id: i64,
    source_actor_id: &str,
    target_actor_id: &str,
    escalated_by_actor_id: &str,
    escalated_at: i64,
) -> Value {
    let mut payload_obj = match source_payload {
        Value::Object(map) => map.clone(),
        _ => Map::new(),
    };
    payload_obj.insert(
        "mailbox_escalation".to_string(),
        serde_json::json!({
            "kind": MAILBOX_RESOLUTION_ESCALATED,
            "source_message_id": source_message_id,
            "source_actor_id": source_actor_id,
            "target_actor_id": target_actor_id,
            "escalated_by_actor_id": escalated_by_actor_id,
            "escalated_at": escalated_at
        }),
    );
    Value::Object(payload_obj)
}

pub(super) fn build_transferred_mailbox_payload(
    source_payload: &Value,
    source_message_id: i64,
    source_actor_id: &str,
    target_actor_id: &str,
    transferred_by_actor_id: &str,
    transferred_at: i64,
) -> Value {
    let mut payload_obj = match source_payload {
        Value::Object(map) => map.clone(),
        _ => Map::new(),
    };
    payload_obj.insert(
        "mailbox_transfer".to_string(),
        serde_json::json!({
            "kind": MAILBOX_RESOLUTION_TRANSFERRED,
            "source_message_id": source_message_id,
            "source_actor_id": source_actor_id,
            "target_actor_id": target_actor_id,
            "transferred_by_actor_id": transferred_by_actor_id,
            "transferred_at": transferred_at
        }),
    );
    Value::Object(payload_obj)
}

pub(super) fn build_taken_over_mailbox_payload(
    source_payload: &Value,
    source_message_id: i64,
    source_actor_id: &str,
    target_actor_id: &str,
    taken_over_by_actor_id: &str,
    taken_over_at: i64,
) -> Value {
    let mut payload_obj = match source_payload {
        Value::Object(map) => map.clone(),
        _ => Map::new(),
    };
    payload_obj.insert(
        "mailbox_takeover".to_string(),
        serde_json::json!({
            "kind": MAILBOX_RESOLUTION_TAKEN_OVER,
            "source_message_id": source_message_id,
            "source_actor_id": source_actor_id,
            "target_actor_id": target_actor_id,
            "taken_over_by_actor_id": taken_over_by_actor_id,
            "taken_over_at": taken_over_at
        }),
    );
    Value::Object(payload_obj)
}
