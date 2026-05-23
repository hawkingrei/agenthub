use super::ActorMailboxImmediateHintReason;

pub(crate) fn build_actor_mailbox_immediate_hint_prompt(
    run_id: &str,
    reason: ActorMailboxImmediateHintReason,
) -> String {
    let headline = match reason {
        ActorMailboxImmediateHintReason::DirectAgentMessage => "Direct mailbox message pending",
        ActorMailboxImmediateHintReason::CoordinatorChannelMention => {
            "Coordinator mentioned you in channel"
        }
    };
    format!("{headline} for run '{run_id}'. Use agenthub actor inbox --run-id \"{run_id}\".")
}

pub(crate) fn build_actor_mailbox_unread_summary_prompt(run_id: &str, unread_count: i64) -> String {
    format!(
        "Mailbox unread summary for run '{run_id}': {unread_count} unread. Use agenthub actor inbox --run-id \"{run_id}\"."
    )
}
