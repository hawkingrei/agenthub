use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use agenthub_team_actor::{ActorIdentityKind, ActorSendResponse};
use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::agent::AgentManager;

use super::TeamManager;

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
struct IdleUnreadHintState {
    session_id: String,
    idle_anchor_ts: i64,
    unread_count: i64,
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
        prompt: &str,
    ) -> anyhow::Result<()> {
        self.send_input(actor_id, prompt, None, expected_session_id)
            .await
    }
}

#[derive(Clone)]
pub struct TeamMailboxUnreadHintWorker {
    teams: Arc<TeamManager>,
    agent_nudger: Arc<dyn TeamMailboxHintAgentNudger>,
    state: Arc<Mutex<HashMap<String, IdleUnreadHintState>>>,
}

impl TeamMailboxUnreadHintWorker {
    pub fn new(teams: Arc<TeamManager>, agents: Arc<AgentManager>) -> Self {
        Self::with_agent_nudger(teams, agents)
    }

    pub fn with_agent_nudger(
        teams: Arc<TeamManager>,
        agent_nudger: Arc<dyn TeamMailboxHintAgentNudger>,
    ) -> Self {
        Self {
            teams,
            agent_nudger,
            state: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn spawn(
        self,
        settings: TeamMailboxUnreadHintWorkerSettings,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(
                settings.poll_interval_secs.max(1) as u64,
            ));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                if let Err(err) = self.dispatch_once(settings).await {
                    tracing::warn!("team mailbox unread hint tick failed: {}", err);
                }
            }
        })
    }

    pub async fn dispatch_once(
        &self,
        settings: TeamMailboxUnreadHintWorkerSettings,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().timestamp();
        let pending = self.teams.list_pending_actor_unread_counts().await?;
        let mut active_keys = HashSet::with_capacity(pending.len());
        let mut next_state = HashMap::new();

        for record in pending {
            if record.unread_count <= 0 {
                continue;
            }
            let Some(runtime) = self
                .agent_nudger
                .running_actor_runtime(record.actor_id.as_str())
                .await
            else {
                continue;
            };
            if runtime.current_run_id.as_deref() != Some(record.run_id.as_str()) {
                continue;
            }
            let Some(idle_anchor_ts) = self
                .agent_nudger
                .mailbox_idle_anchor_ts(record.actor_id.as_str(), runtime.session_id.as_str())
                .await?
            else {
                continue;
            };
            let key = idle_unread_hint_key(&record.run_id, &record.actor_id);
            active_keys.insert(key.clone());

            let previous = {
                let guard = self.state.lock().await;
                guard.get(&key).cloned()
            };

            let Some(action) = decide_idle_unread_hint_action(
                now,
                settings.idle_after_secs,
                runtime.session_id.as_str(),
                idle_anchor_ts,
                record.unread_count,
                previous.as_ref(),
            ) else {
                if let Some(previous) = previous {
                    next_state.insert(key, previous);
                }
                continue;
            };

            let prompt =
                build_actor_mailbox_unread_summary_prompt(&record.run_id, record.unread_count);
            match self
                .agent_nudger
                .nudge_mailbox_prompt(
                    record.actor_id.as_str(),
                    Some(runtime.session_id.as_str()),
                    prompt.as_str(),
                )
                .await
            {
                Ok(()) => {
                    next_state.insert(
                        key,
                        IdleUnreadHintState {
                            session_id: action.session_id,
                            idle_anchor_ts: action.idle_anchor_ts,
                            unread_count: action.unread_count,
                        },
                    );
                }
                Err(err) => {
                    tracing::debug!(
                        actor_id = %record.actor_id,
                        run_id = %record.run_id,
                        unread_count = record.unread_count,
                        "skip idle unread mailbox summary because agent input is unavailable: {}",
                        err
                    );
                    if let Some(previous) = previous {
                        next_state.insert(key, previous);
                    }
                }
            }
        }

        let mut guard = self.state.lock().await;
        guard.retain(|key, _| active_keys.contains(key));
        guard.extend(next_state);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActorMailboxPriorityPlan {
    priority_class: ActorMailboxPriorityClass,
    immediate_hint: Option<ActorMailboxImmediateHintPlan>,
}

async fn resolve_actor_mailbox_priority_plan(
    manager: &TeamManager,
    run_id: &str,
    send_result: &ActorSendResponse,
) -> anyhow::Result<ActorMailboxPriorityPlan> {
    let message = &send_result.message;
    if message.to_actor_kind != ActorIdentityKind::Agent {
        return Ok(ActorMailboxPriorityPlan {
            priority_class: ActorMailboxPriorityClass::General,
            immediate_hint: None,
        });
    }
    if is_channel_payload(&message.payload) {
        let Some(role) = manager
            .member_role_for_run(run_id, &message.from_actor_id)
            .await?
        else {
            return Ok(ActorMailboxPriorityPlan {
                priority_class: ActorMailboxPriorityClass::General,
                immediate_hint: None,
            });
        };
        if !role.eq_ignore_ascii_case("coordinator") {
            return Ok(ActorMailboxPriorityPlan {
                priority_class: ActorMailboxPriorityClass::General,
                immediate_hint: None,
            });
        }
        let mention_targets =
            collect_channel_mention_actor_ids(&message.payload, &message.from_actor_id);
        return Ok(if mention_targets.is_empty() {
            ActorMailboxPriorityPlan {
                priority_class: ActorMailboxPriorityClass::General,
                immediate_hint: None,
            }
        } else {
            ActorMailboxPriorityPlan {
                priority_class: ActorMailboxPriorityClass::Urgent,
                immediate_hint: Some(ActorMailboxImmediateHintPlan {
                    target_actor_ids: mention_targets,
                    reason: ActorMailboxImmediateHintReason::CoordinatorChannelMention,
                }),
            }
        });
    }
    if message.from_actor_kind == ActorIdentityKind::Agent {
        return Ok(ActorMailboxPriorityPlan {
            priority_class: ActorMailboxPriorityClass::Urgent,
            immediate_hint: Some(ActorMailboxImmediateHintPlan {
                target_actor_ids: vec![message.to_actor_id.clone()],
                reason: ActorMailboxImmediateHintReason::DirectAgentMessage,
            }),
        });
    }
    Ok(ActorMailboxPriorityPlan {
        priority_class: ActorMailboxPriorityClass::General,
        immediate_hint: None,
    })
}

pub(crate) async fn plan_actor_mailbox_immediate_hint(
    manager: &TeamManager,
    run_id: &str,
    send_result: &ActorSendResponse,
) -> anyhow::Result<Option<ActorMailboxImmediateHintPlan>> {
    if send_result.deduped {
        return Ok(None);
    }
    Ok(
        resolve_actor_mailbox_priority_plan(manager, run_id, send_result)
            .await?
            .immediate_hint,
    )
}

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

pub(crate) fn actor_mailbox_priority_label(
    priority_class: ActorMailboxPriorityClass,
) -> &'static str {
    match priority_class {
        ActorMailboxPriorityClass::General => "general",
        ActorMailboxPriorityClass::Urgent => "urgent",
        ActorMailboxPriorityClass::PermissionReview => "permission_review",
    }
}

fn is_channel_payload(payload: &Value) -> bool {
    payload
        .as_object()
        .and_then(|obj| obj.get("channel_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

fn collect_channel_mention_actor_ids(payload: &Value, from_actor_id: &str) -> Vec<String> {
    payload
        .as_object()
        .and_then(|obj| {
            obj.get("mentioned_actor_ids")
                .or_else(|| obj.get("mention_actor_ids"))
        })
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|actor_id| !actor_id.is_empty() && *actor_id != from_actor_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn idle_unread_hint_key(run_id: &str, actor_id: &str) -> String {
    format!("{run_id}:{actor_id}")
}

pub(crate) fn actor_mailbox_is_idle(now: i64, idle_after_secs: i64, idle_anchor_ts: i64) -> bool {
    now.saturating_sub(idle_anchor_ts) >= idle_after_secs.max(1)
}

fn decide_idle_unread_hint_action(
    now: i64,
    idle_after_secs: i64,
    session_id: &str,
    idle_anchor_ts: i64,
    unread_count: i64,
    previous: Option<&IdleUnreadHintState>,
) -> Option<IdleUnreadHintAction> {
    if unread_count <= 0 {
        return None;
    }
    if !actor_mailbox_is_idle(now, idle_after_secs, idle_anchor_ts) {
        return None;
    }
    if let Some(previous) = previous
        && previous.session_id == session_id
        && previous.idle_anchor_ts == idle_anchor_ts
        && previous.unread_count == unread_count
    {
        return None;
    }
    Some(IdleUnreadHintAction {
        session_id: session_id.to_string(),
        idle_anchor_ts,
        unread_count,
    })
}

#[cfg(test)]
fn is_user_message_payload(message: &str) -> bool {
    serde_json::from_str::<Value>(message)
        .ok()
        .and_then(|payload| {
            payload
                .get("type")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .is_some_and(|value| value == "user_message")
}

#[cfg(test)]
mod tests {
    use agenthub_team_actor::{
        ACTOR_MAIN_PEER_ID, ActorMessageRecord, ActorMessageStatus, ActorMessageTransport,
    };
    use serde_json::{Value, json};

    use super::{
        ActorMailboxImmediateHintPlan, ActorMailboxImmediateHintReason, ActorMailboxPriorityClass,
        IdleUnreadHintState, actor_mailbox_is_idle, actor_mailbox_priority_label,
        build_actor_mailbox_immediate_hint_prompt, build_actor_mailbox_unread_summary_prompt,
        collect_channel_mention_actor_ids, decide_idle_unread_hint_action, is_user_message_payload,
    };

    fn actor_send_response(
        from_actor_kind: agenthub_team_actor::ActorIdentityKind,
        to_actor_id: &str,
        payload: Value,
    ) -> agenthub_team_actor::ActorSendResponse {
        agenthub_team_actor::ActorSendResponse {
            message_id: 11,
            state: ActorMessageStatus::Pending,
            deduped: false,
            created_at: 100,
            message: ActorMessageRecord {
                message_id: 11,
                run_id: "run-1".to_string(),
                from_actor_id: "planner".to_string(),
                from_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
                from_actor_kind,
                to_actor_id: to_actor_id.to_string(),
                to_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
                to_actor_kind: agenthub_team_actor::ActorIdentityKind::Agent,
                channel: "default".to_string(),
                transport: ActorMessageTransport::Local,
                route: None,
                payload,
                status: ActorMessageStatus::Pending,
                created_at: 100,
                delivered_at: None,
            },
        }
    }

    #[test]
    fn collect_channel_mentions_deduplicates_and_skips_sender() {
        let mentions = collect_channel_mention_actor_ids(
            &json!({
                "channel_id": "all",
                "mentioned_actor_ids": ["reviewer", "planner", "reviewer", "worker"]
            }),
            "planner",
        );
        assert_eq!(mentions, vec!["reviewer".to_string(), "worker".to_string()]);
    }

    #[test]
    fn build_immediate_prompt_is_compact() {
        let prompt = build_actor_mailbox_immediate_hint_prompt(
            "run-42",
            ActorMailboxImmediateHintReason::DirectAgentMessage,
        );
        assert!(prompt.contains("Direct mailbox message pending"));
        assert!(prompt.contains("run-42"));
        assert!(prompt.contains("agenthub actor inbox"));
    }

    #[test]
    fn build_unread_summary_prompt_includes_count() {
        let prompt = build_actor_mailbox_unread_summary_prompt("run-7", 3);
        assert!(prompt.contains("3 unread"));
        assert!(prompt.contains("run-7"));
    }

    #[test]
    fn actor_mailbox_priority_classes_are_stable() {
        assert_eq!(
            actor_mailbox_priority_label(ActorMailboxPriorityClass::General),
            "general"
        );
        assert_eq!(
            actor_mailbox_priority_label(ActorMailboxPriorityClass::Urgent),
            "urgent"
        );
        assert_eq!(
            actor_mailbox_priority_label(ActorMailboxPriorityClass::PermissionReview),
            "permission_review"
        );
    }

    #[test]
    fn actor_mailbox_is_idle_respects_threshold() {
        assert!(actor_mailbox_is_idle(400, 180, 200));
        assert!(!actor_mailbox_is_idle(250, 180, 200));
    }

    #[test]
    fn decide_idle_unread_hint_action_requires_threshold() {
        let action = decide_idle_unread_hint_action(400, 180, "session-1", 200, 2, None)
            .expect("idle unread prompt should trigger");
        assert_eq!(action.session_id, "session-1");
        assert_eq!(action.unread_count, 2);
        assert!(
            decide_idle_unread_hint_action(250, 180, "session-1", 200, 2, None).is_none(),
            "threshold should suppress early prompt"
        );
        assert!(
            decide_idle_unread_hint_action(300, 180, "session-1", 200, 0, None).is_none(),
            "zero unread should suppress prompt"
        );
    }

    #[test]
    fn decide_idle_unread_hint_action_dedupes_same_idle_window() {
        let previous = IdleUnreadHintState {
            session_id: "session-1".to_string(),
            idle_anchor_ts: 200,
            unread_count: 2,
        };
        assert!(
            decide_idle_unread_hint_action(400, 180, "session-1", 200, 2, Some(&previous))
                .is_none()
        );
        assert!(
            decide_idle_unread_hint_action(400, 180, "session-1", 200, 3, Some(&previous))
                .is_some(),
            "count change should retrigger"
        );
        assert!(
            decide_idle_unread_hint_action(400, 180, "session-2", 200, 2, Some(&previous))
                .is_some(),
            "session change should retrigger"
        );
        assert!(
            decide_idle_unread_hint_action(400, 180, "session-1", 260, 2, Some(&previous))
                .is_none(),
            "new idle anchor still needs to satisfy threshold"
        );
        assert!(
            decide_idle_unread_hint_action(500, 180, "session-1", 260, 2, Some(&previous))
                .is_some(),
            "new idle anchor after threshold should retrigger"
        );
    }

    #[test]
    fn detect_user_message_payload() {
        assert!(is_user_message_payload(
            r#"{"type":"user_message","text":"ping"}"#
        ));
        assert!(!is_user_message_payload(
            r#"{"type":"agent_message","text":"pong"}"#
        ));
        assert!(!is_user_message_payload("not-json"));
    }

    #[test]
    fn direct_agent_message_shape_matches_immediate_contract() {
        let response = actor_send_response(
            agenthub_team_actor::ActorIdentityKind::Agent,
            "reviewer",
            json!({"type":"chat_message","text":"hello"}),
        );
        let plan = ActorMailboxImmediateHintPlan {
            target_actor_ids: vec![response.message.to_actor_id.clone()],
            reason: ActorMailboxImmediateHintReason::DirectAgentMessage,
        };
        assert_eq!(plan.target_actor_ids, vec!["reviewer".to_string()]);
    }
}
