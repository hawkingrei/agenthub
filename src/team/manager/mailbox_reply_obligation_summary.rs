use std::collections::HashMap;

use super::TeamReplyObligationSummary;
use super::mailbox::ReplyActorPairKey;
use super::mailbox_reply_obligation_payloads::{
    build_reply_obligation_record_from_snapshot, reply_actor_pair_for_inbound_obligation_snapshot,
    reply_actor_pair_for_visible_reply_snapshot, reply_obligation_snapshot_is_terminal,
};
use super::mailbox_reply_obligation_snapshot_conversion::reply_obligation_snapshot_from_message;
use super::mailbox_reply_obligation_snapshots::ReplyObligationMessageSnapshot;
pub(super) use super::mailbox_reply_obligation_snapshots::load_reply_obligation_message_snapshots_on_executor;
use crate::team::TeamActorMessageRecord;

pub(super) fn summarize_open_reply_obligations_from_snapshots(
    messages: &[ReplyObligationMessageSnapshot],
) -> TeamReplyObligationSummary {
    let mut visible_reply_credits = HashMap::<ReplyActorPairKey, i64>::new();
    let mut summary = TeamReplyObligationSummary::default();

    for message in messages.iter().rev() {
        if let Some(pair_key) = reply_actor_pair_for_visible_reply_snapshot(message) {
            *visible_reply_credits.entry(pair_key).or_default() += 1;
            continue;
        }
        let Some(pair_key) = reply_actor_pair_for_inbound_obligation_snapshot(message) else {
            continue;
        };
        if reply_obligation_snapshot_is_terminal(message) {
            continue;
        }
        if let Some(credits) = visible_reply_credits.get_mut(&pair_key)
            && *credits > 0
        {
            *credits -= 1;
            continue;
        }
        *summary
            .open_by_actor
            .entry(pair_key.agent_actor_id.clone())
            .or_default() += 1;
        summary.open_total += 1;
        summary
            .open_items
            .push(build_reply_obligation_record_from_snapshot(
                message, pair_key,
            ));
    }

    summary
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn summarize_open_reply_obligations_from_messages(
    messages: &[TeamActorMessageRecord],
) -> TeamReplyObligationSummary {
    let snapshots = messages
        .iter()
        .map(reply_obligation_snapshot_from_message)
        .collect::<Vec<_>>();
    summarize_open_reply_obligations_from_snapshots(snapshots.as_slice())
}

pub(super) fn has_visible_reply_credit_for_message(
    messages: &[ReplyObligationMessageSnapshot],
    target_message_id: i64,
) -> bool {
    let mut visible_reply_credits = HashMap::<ReplyActorPairKey, i64>::new();
    for message in messages.iter().rev() {
        if let Some(pair_key) = reply_actor_pair_for_visible_reply_snapshot(message) {
            *visible_reply_credits.entry(pair_key).or_default() += 1;
            continue;
        }
        let Some(pair_key) = reply_actor_pair_for_inbound_obligation_snapshot(message) else {
            continue;
        };
        if reply_obligation_snapshot_is_terminal(message) {
            continue;
        }
        if let Some(credits) = visible_reply_credits.get_mut(&pair_key)
            && *credits > 0
        {
            if message.message_id == target_message_id {
                return true;
            }
            *credits -= 1;
            continue;
        }
        if message.message_id == target_message_id {
            return false;
        }
    }
    true
}
