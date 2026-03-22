use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ActorIdentityKind, ActorMessageTransport, ActorSendResponse,
};
use serde_json::Value;

use super::TeamManager;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ActorMailboxTypeHintPlan {
    Suppressed {
        message_id: i64,
        to_actor_id: String,
        payload_type: String,
        reason: &'static str,
    },
    Notify {
        message_id: i64,
        to_actor_id: String,
        payload_type: String,
        prompt: String,
    },
}

pub(crate) async fn plan_actor_mailbox_type_hint(
    manager: &TeamManager,
    run_id: &str,
    send_result: &ActorSendResponse,
) -> anyhow::Result<Option<ActorMailboxTypeHintPlan>> {
    if send_result.deduped {
        return Ok(None);
    }
    let message = &send_result.message;
    if message.transport != ActorMessageTransport::Local {
        return Ok(None);
    }
    if message.to_peer_id != ACTOR_MAIN_PEER_ID {
        return Ok(None);
    }
    if message.to_actor_kind != ActorIdentityKind::Agent {
        return Ok(None);
    }
    let Some(payload_type) = extract_mailbox_payload_type(&message.payload) else {
        return Ok(None);
    };
    if should_suppress_mailbox_type_hint_for_pending_same_type(payload_type.as_str()) {
        let has_pending_same_type = manager
            .has_pending_actor_message_payload_type(
                run_id,
                &message.to_actor_id,
                payload_type.as_str(),
                Some(message.message_id),
            )
            .await?;
        if has_pending_same_type {
            return Ok(Some(ActorMailboxTypeHintPlan::Suppressed {
                message_id: message.message_id,
                to_actor_id: message.to_actor_id.clone(),
                payload_type,
                reason: "pending_same_type_exists",
            }));
        }
    }
    Ok(Some(ActorMailboxTypeHintPlan::Notify {
        message_id: message.message_id,
        to_actor_id: message.to_actor_id.clone(),
        payload_type: payload_type.clone(),
        prompt: build_actor_mailbox_type_hint_prompt(run_id, payload_type.as_str()),
    }))
}

pub(crate) fn extract_mailbox_payload_type(payload: &Value) -> Option<String> {
    let payload_type = payload
        .as_object()
        .and_then(|obj| obj.get("type"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    Some(payload_type.to_string())
}

pub(crate) fn build_actor_mailbox_type_hint_prompt(run_id: &str, payload_type: &str) -> String {
    format!(
        "New mailbox message type '{payload_type}' is pending in run '{run_id}'. Use $AGENTHUB_ACTOR_CLI actor inbox to inspect pending messages and batch-handle this type before ack."
    )
}

pub(crate) fn should_suppress_mailbox_type_hint_for_pending_same_type(payload_type: &str) -> bool {
    !payload_type.trim().eq_ignore_ascii_case("chat_message")
}
