use async_trait::async_trait;

use crate::agent::AgentManager;

pub(crate) const DEFAULT_TEAM_MAILBOX_IDLE_AFTER_SECS: i64 = 180;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActorMailboxPriorityClass {
    General,
    Urgent,
    PermissionReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActorMailboxImmediateHintReason {
    DirectAgentMessage,
    CoordinatorChannelMention,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActorMailboxImmediateHintPlan {
    pub target_actor_ids: Vec<String>,
    pub reason: ActorMailboxImmediateHintReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActorMailboxImmediateHintDelivery {
    pub delivery_ids: Vec<String>,
    pub sent_actor_ids: Vec<String>,
    pub failed_actor_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct TeamMailboxUnreadHintWorkerSettings {
    pub poll_interval_secs: i64,
    pub idle_after_secs: i64,
}

impl Default for TeamMailboxUnreadHintWorkerSettings {
    fn default() -> Self {
        Self {
            poll_interval_secs: 30,
            idle_after_secs: DEFAULT_TEAM_MAILBOX_IDLE_AFTER_SECS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunningActorRuntime {
    pub session_id: String,
    pub current_run_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IdleUnreadHintAction {
    pub session_id: String,
    pub idle_anchor_ts: i64,
    pub unread_count: i64,
}

#[async_trait]
pub trait TeamMailboxHintAgentNudger: Send + Sync {
    async fn running_actor_runtime(&self, actor_id: &str) -> Option<RunningActorRuntime>;

    async fn mailbox_idle_anchor_ts(
        &self,
        actor_id: &str,
        session_id: &str,
    ) -> anyhow::Result<Option<i64>>;

    async fn nudge_mailbox_prompt(
        &self,
        actor_id: &str,
        expected_session_id: Option<&str>,
        delivery_id: &str,
        prompt: &str,
    ) -> anyhow::Result<()>;
}

#[async_trait]
impl TeamMailboxHintAgentNudger for AgentManager {
    async fn running_actor_runtime(&self, actor_id: &str) -> Option<RunningActorRuntime> {
        let session_id = self.running_session_id_for_agent(actor_id).await?;
        let current_run_id = self
            .running_actor_context_for_agent(actor_id)
            .await
            .and_then(|context| context.current_run_id);
        Some(RunningActorRuntime {
            session_id,
            current_run_id,
        })
    }

    async fn mailbox_idle_anchor_ts(
        &self,
        actor_id: &str,
        session_id: &str,
    ) -> anyhow::Result<Option<i64>> {
        self.mailbox_idle_anchor_ts(actor_id, session_id).await
    }

    async fn nudge_mailbox_prompt(
        &self,
        actor_id: &str,
        expected_session_id: Option<&str>,
        delivery_id: &str,
        prompt: &str,
    ) -> anyhow::Result<()> {
        self.send_mailbox_hint_input(actor_id, prompt, expected_session_id, delivery_id)
            .await
    }
}

pub(crate) fn actor_mailbox_priority_label(
    priority_class: ActorMailboxPriorityClass,
) -> &'static str {
    match priority_class {
        ActorMailboxPriorityClass::General => "general",
        ActorMailboxPriorityClass::Urgent => "urgent",
        ActorMailboxPriorityClass::PermissionReview => "permission_review",
    }
}
