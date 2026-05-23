mod immediate;
mod prompts;
mod types;
mod worker;

pub(crate) use immediate::{
    dispatch_actor_mailbox_immediate_hint, plan_actor_mailbox_immediate_hint,
};
#[cfg(test)]
pub(crate) use prompts::build_actor_mailbox_immediate_hint_prompt;
#[allow(unused_imports)]
pub(crate) use types::{
    ActorMailboxImmediateHintReason, ActorMailboxPriorityClass,
    DEFAULT_TEAM_MAILBOX_IDLE_AFTER_SECS, RunningActorRuntime, TeamMailboxHintAgentNudger,
    TeamMailboxUnreadHintWorkerSettings, actor_mailbox_priority_label,
};
pub use worker::TeamMailboxUnreadHintWorker;
#[allow(unused_imports)]
pub(crate) use worker::actor_mailbox_is_idle;

#[cfg(test)]
mod tests;
