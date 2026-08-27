use std::sync::Arc;
use std::time::Duration;

use super::prompts::build_actor_mailbox_immediate_hint_prompt;
use super::types::{
    ActorMailboxImmediateHintDelivery, ActorMailboxImmediateHintPlan, TeamMailboxHintAgentNudger,
};
use crate::team::{TeamManager, TeamRuntimeDeliveryReceipt};

#[derive(Debug, Clone, Copy)]
pub struct TeamMailboxRuntimeDeliveryWorkerSettings {
    pub poll_interval: Duration,
    pub batch_size: i64,
    pub lease_seconds: i64,
}

impl Default for TeamMailboxRuntimeDeliveryWorkerSettings {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(5),
            batch_size: 100,
            lease_seconds: 30,
        }
    }
}

#[derive(Clone)]
pub struct TeamMailboxRuntimeDeliveryWorker {
    teams: Arc<TeamManager>,
    agent_nudger: Arc<dyn TeamMailboxHintAgentNudger>,
}

impl TeamMailboxRuntimeDeliveryWorker {
    pub fn new(teams: Arc<TeamManager>, agents: Arc<crate::agent::AgentManager>) -> Self {
        Self::with_agent_nudger(teams, agents)
    }

    pub fn with_agent_nudger(
        teams: Arc<TeamManager>,
        agent_nudger: Arc<dyn TeamMailboxHintAgentNudger>,
    ) -> Self {
        Self {
            teams,
            agent_nudger,
        }
    }

    pub fn spawn(
        self,
        settings: TeamMailboxRuntimeDeliveryWorkerSettings,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(settings.poll_interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                if let Err(error) = self.dispatch_once(settings).await {
                    tracing::warn!(error = %error, "team mailbox runtime delivery tick failed");
                }
            }
        })
    }

    pub(crate) async fn enqueue_and_dispatch(
        &self,
        run_id: &str,
        message_id: i64,
        plan: &ActorMailboxImmediateHintPlan,
    ) -> anyhow::Result<ActorMailboxImmediateHintDelivery> {
        let prompt = build_actor_mailbox_immediate_hint_prompt(run_id, plan.reason);
        self.enqueue_prompt(run_id, message_id, &plan.target_actor_ids, &prompt)
            .await
    }

    pub(crate) async fn enqueue_prompt(
        &self,
        run_id: &str,
        message_id: i64,
        actor_ids: &[String],
        prompt: &str,
    ) -> anyhow::Result<ActorMailboxImmediateHintDelivery> {
        let now = chrono::Utc::now().timestamp();
        let receipts = self
            .teams
            .ensure_mailbox_runtime_deliveries(run_id, message_id, actor_ids, prompt, now)
            .await?;
        self.dispatch_receipts(
            receipts,
            now,
            TeamMailboxRuntimeDeliveryWorkerSettings::default(),
        )
        .await
    }

    pub async fn dispatch_once(
        &self,
        settings: TeamMailboxRuntimeDeliveryWorkerSettings,
    ) -> anyhow::Result<ActorMailboxImmediateHintDelivery> {
        let now = chrono::Utc::now().timestamp();
        self.dispatch_once_at(settings, now).await
    }

    pub(crate) async fn dispatch_once_at(
        &self,
        settings: TeamMailboxRuntimeDeliveryWorkerSettings,
        now: i64,
    ) -> anyhow::Result<ActorMailboxImmediateHintDelivery> {
        let receipts = self
            .teams
            .list_due_mailbox_runtime_deliveries(now, settings.batch_size)
            .await?;
        self.dispatch_receipts(receipts, now, settings).await
    }

    async fn dispatch_receipts(
        &self,
        receipts: Vec<TeamRuntimeDeliveryReceipt>,
        now: i64,
        settings: TeamMailboxRuntimeDeliveryWorkerSettings,
    ) -> anyhow::Result<ActorMailboxImmediateHintDelivery> {
        let mut delivery_ids = Vec::with_capacity(receipts.len());
        let mut sent_actor_ids = Vec::new();
        let mut failed_actor_ids = Vec::new();
        for receipt in receipts {
            delivery_ids.push(receipt.delivery_id.clone());
            if self
                .dispatch_receipt(receipt.clone(), now, settings)
                .await?
            {
                sent_actor_ids.push(receipt.actor_id);
            } else {
                failed_actor_ids.push(receipt.actor_id);
            }
        }
        Ok(ActorMailboxImmediateHintDelivery {
            delivery_ids,
            sent_actor_ids,
            failed_actor_ids,
        })
    }

    async fn dispatch_receipt(
        &self,
        receipt: TeamRuntimeDeliveryReceipt,
        now: i64,
        settings: TeamMailboxRuntimeDeliveryWorkerSettings,
    ) -> anyhow::Result<bool> {
        let Some(claimed) = self
            .teams
            .claim_mailbox_runtime_delivery(&receipt.delivery_id, now, settings.lease_seconds)
            .await?
        else {
            return Ok(receipt.state == "delivered");
        };
        let Some(runtime) = self
            .agent_nudger
            .running_actor_runtime(&claimed.actor_id)
            .await
        else {
            self.schedule_retry(&claimed, now, "actor runtime is not running")
                .await?;
            return Ok(false);
        };
        let delivery_timeout = Duration::from_secs(settings.lease_seconds.max(1) as u64);
        match tokio::time::timeout(
            delivery_timeout,
            self.agent_nudger.nudge_mailbox_prompt(
                &claimed.actor_id,
                Some(&runtime.session_id),
                &claimed.delivery_id,
                &claimed.prompt,
            ),
        )
        .await
        {
            Ok(Ok(())) => {
                let acknowledged = self
                    .teams
                    .acknowledge_mailbox_runtime_delivery(
                        &claimed.delivery_id,
                        claimed.attempt,
                        &runtime.session_id,
                        now,
                    )
                    .await?;
                if !acknowledged {
                    tracing::debug!(
                        delivery_id = %claimed.delivery_id,
                        attempt = claimed.attempt,
                        "team mailbox runtime delivery acknowledgement lost its attempt fence"
                    );
                }
                Ok(acknowledged)
            }
            Ok(Err(error)) => {
                self.schedule_retry(&claimed, now, &error.to_string())
                    .await?;
                Ok(false)
            }
            Err(_) => {
                self.schedule_retry(&claimed, now, "runtime input delivery timed out")
                    .await?;
                Ok(false)
            }
        }
    }

    async fn schedule_retry(
        &self,
        claimed: &TeamRuntimeDeliveryReceipt,
        now: i64,
        error: &str,
    ) -> anyhow::Result<()> {
        let delay = crate::team::runtime_delivery_retry_delay_seconds(claimed.attempt);
        let scheduled = self
            .teams
            .retry_mailbox_runtime_delivery(
                &claimed.delivery_id,
                claimed.attempt,
                now,
                now.saturating_add(delay),
                error,
            )
            .await?;
        if !scheduled {
            tracing::debug!(
                delivery_id = %claimed.delivery_id,
                attempt = claimed.attempt,
                "team mailbox runtime delivery retry lost its attempt fence"
            );
            return Ok(());
        }
        tracing::debug!(
            delivery_id = %claimed.delivery_id,
            run_id = %claimed.run_id,
            actor_id = %claimed.actor_id,
            attempt = claimed.attempt,
            retry_delay_seconds = delay,
            error,
            "team mailbox runtime delivery scheduled for retry"
        );
        Ok(())
    }
}
