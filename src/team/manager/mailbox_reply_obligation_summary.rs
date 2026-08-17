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

/// The untagged fallback bucket for a pair key: replies that carry no thread/conversation info of
/// their own land here, and a thread-scoped obligation may still draw on it (but never on another
/// thread's scoped credits) -- this keeps plain, unthreaded replies working exactly as before while
/// still preventing a reply that *does* declare a thread from satisfying an obligation in a different
/// one.
fn loose_pair_key(pair_key: &ReplyActorPairKey) -> ReplyActorPairKey {
    ReplyActorPairKey {
        agent_actor_id: pair_key.agent_actor_id.clone(),
        human_actor_id: pair_key.human_actor_id.clone(),
        thread_scope: None,
    }
}

/// Consumes one credit for `pair_key`, preferring an exact thread-scoped match and falling back to
/// the untagged pool only when the obligation itself is thread-scoped (an untagged obligation already
/// uses the loose pool as its primary key).
fn consume_reply_credit(
    credits: &mut HashMap<ReplyActorPairKey, i64>,
    pair_key: &ReplyActorPairKey,
) -> bool {
    if let Some(count) = credits.get_mut(pair_key)
        && *count > 0
    {
        *count -= 1;
        return true;
    }
    if pair_key.thread_scope.is_some() {
        let loose_key = loose_pair_key(pair_key);
        if let Some(count) = credits.get_mut(&loose_key)
            && *count > 0
        {
            *count -= 1;
            return true;
        }
    }
    false
}

fn has_reply_credit(
    credits: &HashMap<ReplyActorPairKey, i64>,
    pair_key: &ReplyActorPairKey,
) -> bool {
    if credits.get(pair_key).is_some_and(|count| *count > 0) {
        return true;
    }
    pair_key.thread_scope.is_some()
        && credits
            .get(&loose_pair_key(pair_key))
            .is_some_and(|count| *count > 0)
}

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
        if consume_reply_credit(&mut visible_reply_credits, &pair_key) {
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
            if message.message_id == target_message_id {
                return has_reply_credit(&visible_reply_credits, &pair_key);
            }
            continue;
        }
        if message.message_id == target_message_id {
            return has_reply_credit(&visible_reply_credits, &pair_key);
        }
        consume_reply_credit(&mut visible_reply_credits, &pair_key);
    }
    false
}
