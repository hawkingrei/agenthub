use std::sync::Arc;
use std::time::Duration;

use agenthub_acp::{
    AcpPermissionReviewDispatcher as AcpPermissionReviewDispatcherTrait,
    AcpPermissionReviewRequest, AcpPermissionService,
};
use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ActorMailboxService, ActorMessageTransport, ActorSendRequest,
};
use serde_json::{Value, json};

use crate::agent::AgentManager;
use crate::team::{TeamMailboxRuntimeDeliveryWorker, TeamManager};

use super::payload::{
    build_permission_review_payload, build_permission_review_summary, extract_tool_name,
    human_review_reason_text,
};
use super::selection::collect_team_permission_review_candidates;
use super::{
    DEFAULT_TEAM_PERMISSION_REVIEW_HUMAN_FALLBACK_DELAY, TEAM_HUMAN_PERMISSION_CARD_PAYLOAD_TYPE,
    TEAM_PERMISSION_REVIEW_PAYLOAD_TYPE,
};
use crate::team::mailbox_hint::{
    ActorMailboxPriorityClass, DEFAULT_TEAM_MAILBOX_IDLE_AFTER_SECS, TeamMailboxHintAgentNudger,
    actor_mailbox_is_idle, actor_mailbox_priority_label,
};

#[derive(Debug, Clone, Copy)]
pub struct TeamPermissionReviewDispatcherSettings {
    pub human_fallback_delay: Duration,
}

impl Default for TeamPermissionReviewDispatcherSettings {
    fn default() -> Self {
        Self {
            human_fallback_delay: DEFAULT_TEAM_PERMISSION_REVIEW_HUMAN_FALLBACK_DELAY,
        }
    }
}

#[derive(Clone)]
pub struct TeamPermissionReviewDispatcher {
    teams: Arc<TeamManager>,
    agent_nudger: Arc<dyn TeamMailboxHintAgentNudger>,
    permissions: Arc<AcpPermissionService>,
    settings: TeamPermissionReviewDispatcherSettings,
    daemon_tasks: Option<crate::daemon_tasks::DaemonTaskGroup>,
}

impl TeamPermissionReviewDispatcher {
    pub fn new(
        teams: Arc<TeamManager>,
        agents: Arc<AgentManager>,
        permissions: Arc<AcpPermissionService>,
        settings: TeamPermissionReviewDispatcherSettings,
    ) -> Self {
        let daemon_tasks = agents.daemon_tasks().clone();
        Self {
            teams,
            agent_nudger: agents,
            permissions,
            settings,
            daemon_tasks: Some(daemon_tasks),
        }
    }

    #[cfg(test)]
    pub fn with_agent_nudger(
        teams: Arc<TeamManager>,
        agent_nudger: Arc<dyn TeamMailboxHintAgentNudger>,
        permissions: Arc<AcpPermissionService>,
        settings: TeamPermissionReviewDispatcherSettings,
    ) -> Self {
        Self {
            teams,
            agent_nudger,
            permissions,
            settings,
            daemon_tasks: None,
        }
    }

    async fn dispatch_to_review_target(
        &self,
        request: &AcpPermissionReviewRequest,
    ) -> anyhow::Result<()> {
        let Some(team_id) = request.routing.team_id.as_deref() else {
            return Ok(());
        };
        let requester_actor_id = request
            .routing
            .requester_actor_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("permission review routing requires requester actor"))?;
        let requester_role = request
            .routing
            .requester_role
            .as_deref()
            .map(str::trim)
            .unwrap_or("");
        let team = self.teams.get_team(team_id).await?;
        let (review_target_actor_id, dispatch_status) = self
            .resolve_review_target(&team.spec, requester_actor_id, requester_role)
            .await?;

        let (task_id, conversation_id) = self
            .teams
            .ensure_shared_thread_target_for_team(team_id, requester_actor_id)
            .await?;
        let run_id = if let Some(current_run_id) = request
            .current_run_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            current_run_id.to_string()
        } else {
            self.teams
                .ensure_shared_thread_mailbox_run(team_id, &task_id, &conversation_id)
                .await?
                .id
        };
        let payload = build_permission_review_payload(request, review_target_actor_id.as_str());
        let response = self
            .teams
            .actor_mailbox_service()
            .actor_send(ActorSendRequest {
                run_id: run_id.clone(),
                from_actor_id: requester_actor_id.to_string(),
                from_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
                to_actor_id: Some(review_target_actor_id.clone()),
                channel_id: None,
                to_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
                channel: Some("default".to_string()),
                transport: Some(ActorMessageTransport::Local),
                route: None,
                payload,
                idempotency_key: Some(format!(
                    "permission-review:{}:{}",
                    request.request_id, review_target_actor_id
                )),
                message_kind: None,
            })
            .await
            .map_err(|err| {
                anyhow::anyhow!(
                    "dispatch permission review to reviewer failed: {}",
                    err.message
                )
            })?;
        self.permissions
            .record_review_dispatch(
                &request.request_id,
                Some(review_target_actor_id.as_str()),
                dispatch_status,
                Some(run_id.as_str()),
                Some(response.message_id),
            )
            .await?;
        self.nudge_actor(
            review_target_actor_id.as_str(),
            &run_id,
            response.message_id,
            TEAM_PERMISSION_REVIEW_PAYLOAD_TYPE,
        )
        .await;
        self.spawn_human_review_fallback(request.clone());
        Ok(())
    }

    fn spawn_human_review_fallback(&self, request: AcpPermissionReviewRequest) {
        let dispatcher = self.clone();
        let Some(daemon_tasks) = self.daemon_tasks.as_ref() else {
            tokio::spawn(async move {
                tokio::time::sleep(dispatcher.settings.human_fallback_delay).await;
                let _ = dispatcher
                    .notify_human_review_if_pending(&request, "review_timeout")
                    .await;
            });
            return;
        };
        let cancellation = daemon_tasks.background_cancellation();
        let task_name = format!("permission-review-fallback:{}", request.request_id);
        if let Err(error) = daemon_tasks.spawn_background_job(task_name, async move {
            tokio::select! {
                _ = cancellation.cancelled() => Ok(()),
                _ = tokio::time::sleep(dispatcher.settings.human_fallback_delay) => {
                    dispatcher
                        .notify_human_review_if_pending(&request, "review_timeout")
                        .await
                }
            }
        }) {
            tracing::warn!(error = %error, "permission review fallback rejected during shutdown");
        }
    }

    async fn nudge_actor(&self, actor_id: &str, run_id: &str, message_id: i64, payload_type: &str) {
        let priority_label =
            actor_mailbox_priority_label(ActorMailboxPriorityClass::PermissionReview);
        let hint = format!(
            "New {priority_label} mailbox message type '{payload_type}' is pending in run '{run_id}'. Use agenthub actor inbox --run-id \"{run_id}\" to inspect pending messages and batch-handle this type before ack."
        );
        if let Err(err) = TeamMailboxRuntimeDeliveryWorker::with_agent_nudger(
            self.teams.clone(),
            self.agent_nudger.clone(),
        )
        .enqueue_prompt(run_id, message_id, &[actor_id.to_string()], &hint)
        .await
        {
            tracing::debug!(
                actor_id = %actor_id,
                run_id = %run_id,
                payload_type = %payload_type,
                "skip permission review mailbox hint because agent input is unavailable: {}",
                err
            );
        }
    }

    async fn resolve_review_target(
        &self,
        spec: &Value,
        requester_actor_id: &str,
        requester_role: &str,
    ) -> anyhow::Result<(String, &'static str)> {
        let candidates =
            collect_team_permission_review_candidates(spec, requester_actor_id, requester_role)?;
        let now = chrono::Utc::now().timestamp();
        for candidate in &candidates {
            if self.actor_is_idle(candidate.actor_id.as_str(), now).await? {
                return Ok((candidate.actor_id.clone(), candidate.idle_dispatch_status));
            }
        }
        let fallback = candidates
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("team has no non-requester reviewer configured"))?;
        Ok((fallback.actor_id, fallback.dispatch_status))
    }

    async fn actor_is_idle(&self, actor_id: &str, now: i64) -> anyhow::Result<bool> {
        let Some(runtime) = self.agent_nudger.running_actor_runtime(actor_id).await else {
            return Ok(false);
        };
        let Some(idle_anchor_ts) = self
            .agent_nudger
            .mailbox_idle_anchor_ts(actor_id, runtime.session_id.as_str())
            .await?
        else {
            return Ok(false);
        };
        Ok(actor_mailbox_is_idle(
            now,
            DEFAULT_TEAM_MAILBOX_IDLE_AFTER_SECS,
            idle_anchor_ts,
        ))
    }

    pub(super) async fn notify_human_review_if_pending(
        &self,
        request: &AcpPermissionReviewRequest,
        reason: &str,
    ) -> anyhow::Result<()> {
        let Some(team_id) = request.routing.team_id.as_deref() else {
            return Ok(());
        };
        let Some(record) = self.permissions.get(&request.request_id).await? else {
            return Ok(());
        };
        if record.status != "pending" || record.human_review_notified_at.is_some() {
            return Ok(());
        }
        let review_delivery = record
            .review_delivery_run_id
            .as_deref()
            .zip(record.review_target_actor_id.as_deref())
            .zip(record.review_message_id)
            .map(|((run_id, actor_id), message_id)| {
                (run_id.to_string(), actor_id.to_string(), message_id)
            });
        let requester_actor_id = request
            .routing
            .requester_actor_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("coordinator");
        let (task_id, _) = self
            .teams
            .ensure_shared_thread_target_for_team(team_id, requester_actor_id)
            .await?;
        let payload = json!({
            "type": TEAM_HUMAN_PERMISSION_CARD_PAYLOAD_TYPE,
            "permission_id": request.request_id,
            "agent_id": request.agent_id,
            "agent_session_id": request.agent_session_id,
            "acp_session_id": request.acp_session_id,
            "tool_call_id": request.tool_call_id,
            "tool_call": request.tool_call,
            "tool_name": request.tool_call.as_ref().and_then(extract_tool_name),
            "requester_actor_id": request.routing.requester_actor_id,
            "requester_role": request.routing.requester_role,
            "options": request.options,
            "summary": build_permission_review_summary(request),
            "reason": reason,
            "reason_text": human_review_reason_text(reason),
            "status": "pending",
        });
        let _ = self
            .teams
            .append_task_conversation_message(
                &task_id,
                requester_actor_id,
                None,
                "group_chat",
                payload,
            )
            .await?;
        if !self
            .permissions
            .mark_human_review_notified(&request.request_id)
            .await?
        {
            tracing::debug!(
                permission_id = %request.request_id,
                "human review notification was already marked while appending fallback message"
            );
        }
        if let Some((run_id, actor_id, message_id)) = review_delivery
            && let Err(err) = self
                .teams
                .ack_actor_message(&run_id, &actor_id, message_id)
                .await
        {
            tracing::debug!(
                permission_id = %request.request_id,
                run_id = %run_id,
                actor_id = %actor_id,
                message_id,
                error = %err,
                "failed to ack stale permission review mailbox message after human fallback"
            );
        }
        self.permissions
            .record_review_dispatch(&request.request_id, None, reason, None, None)
            .await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl AcpPermissionReviewDispatcherTrait for TeamPermissionReviewDispatcher {
    async fn dispatch_review(&self, request: AcpPermissionReviewRequest) -> anyhow::Result<()> {
        if let Err(err) = self.dispatch_to_review_target(&request).await {
            tracing::warn!(
                permission_id = %request.request_id,
                error = %err,
                "team permission review dispatch failed; falling back to human review"
            );
            let _ = self
                .permissions
                .record_review_dispatch(
                    &request.request_id,
                    None,
                    "review_dispatch_failed",
                    None,
                    None,
                )
                .await;
            let _ = self
                .notify_human_review_if_pending(&request, "review_dispatch_failed")
                .await;
        }
        Ok(())
    }
}
