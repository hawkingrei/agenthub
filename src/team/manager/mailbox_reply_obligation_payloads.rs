use super::mailbox::{
    MAILBOX_RESOLUTION_ESCALATED, MAILBOX_RESOLUTION_IGNORED, MAILBOX_RESOLUTION_TAKEN_OVER,
    MAILBOX_RESOLUTION_TRANSFERRED, ReplyActorPairKey, reply_thread_scope,
};
use super::mailbox_payloads::is_human_actor_id;
use super::mailbox_reply_obligation_snapshot_conversion::resolve_canonical_chat_reply_from_snapshot;
use super::mailbox_reply_obligation_snapshots::ReplyObligationMessageSnapshot;
use crate::team::{TeamActorMessageStatus, TeamActorMessageTransport, TeamReplyObligationRecord};
use agenthub_team_actor::{ACTOR_MAIN_PEER_ID, ActorMessageHandlingDisposition, ActorMessageKind};

pub(super) fn reply_actor_pair_for_inbound_obligation_snapshot(
    message: &ReplyObligationMessageSnapshot,
) -> Option<ReplyActorPairKey> {
    if !(message.requires_user_visible_reply
        || (message.message_kind == ActorMessageKind::HumanRequest
            && is_human_actor_id(&message.from_actor_id)
            && !is_human_actor_id(&message.to_actor_id)))
    {
        return None;
    }
    if !is_human_actor_id(&message.from_actor_id) || is_human_actor_id(&message.to_actor_id) {
        return None;
    }
    Some(ReplyActorPairKey {
        agent_actor_id: message.to_actor_id.clone(),
        human_actor_id: message.from_actor_id.clone(),
        thread_scope: reply_thread_scope(
            message.conversation_id.as_deref(),
            message.thread_root_message_id,
        ),
    })
}

pub(super) fn reply_actor_pair_for_visible_reply_snapshot(
    message: &ReplyObligationMessageSnapshot,
) -> Option<ReplyActorPairKey> {
    if message.status == TeamActorMessageStatus::DeadLetter
        || message.transport != TeamActorMessageTransport::Local
        || message.to_peer_id != ACTOR_MAIN_PEER_ID
        || !is_human_actor_id(&message.to_actor_id)
        || is_human_actor_id(&message.from_actor_id)
        || resolve_canonical_chat_reply_from_snapshot(message).is_none()
    {
        return None;
    }
    Some(ReplyActorPairKey {
        agent_actor_id: message.from_actor_id.clone(),
        human_actor_id: message.to_actor_id.clone(),
        thread_scope: reply_thread_scope(
            message.conversation_id.as_deref(),
            message.thread_root_message_id,
        ),
    })
}

pub(super) fn build_reply_obligation_record_from_snapshot(
    message: &ReplyObligationMessageSnapshot,
    pair_key: ReplyActorPairKey,
) -> TeamReplyObligationRecord {
    TeamReplyObligationRecord {
        message_id: message.message_id,
        agent_actor_id: pair_key.agent_actor_id,
        human_actor_id: pair_key.human_actor_id,
        source_surface: reply_obligation_source_surface(message),
        reply_target: message.reply_target.clone(),
        conversation_id: message.conversation_id.clone(),
        thread_root_message_id: message.thread_root_message_id,
        text_excerpt: resolve_canonical_chat_reply_from_snapshot(message).map(|reply| reply.text),
        created_at: message.created_at,
    }
}

pub(super) fn reply_obligation_snapshot_is_terminal(
    message: &ReplyObligationMessageSnapshot,
) -> bool {
    if message.handling_disposition == ActorMessageHandlingDisposition::Ignored {
        return message.mailbox_resolution_kind.as_deref() == Some(MAILBOX_RESOLUTION_IGNORED)
            && message
                .mailbox_resolution_reason
                .as_deref()
                .is_some_and(|reason| !reason.trim().is_empty());
    }
    matches!(
        message.handling_disposition,
        ActorMessageHandlingDisposition::Released
    ) && matches!(
        message.mailbox_resolution_kind.as_deref(),
        Some(
            MAILBOX_RESOLUTION_ESCALATED
                | MAILBOX_RESOLUTION_TRANSFERRED
                | MAILBOX_RESOLUTION_TAKEN_OVER,
        )
    )
}

fn reply_obligation_source_surface(message: &ReplyObligationMessageSnapshot) -> String {
    if let Some(source_surface) = message.source_surface.as_deref() {
        return source_surface.to_string();
    }
    if message.thread_root_message_id.is_some() {
        return "thread".to_string();
    }
    if message.conversation_id.is_some() {
        return "conversation".to_string();
    }
    match message.message_kind {
        ActorMessageKind::TriggerEvent => "trigger".to_string(),
        ActorMessageKind::SystemNotice => "system".to_string(),
        _ => "mailbox".to_string(),
    }
}
