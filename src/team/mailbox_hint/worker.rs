use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use crate::agent::AgentManager;

use super::prompts::build_actor_mailbox_unread_summary_prompt;
use super::types::{
    IdleUnreadHintAction, TeamMailboxHintAgentNudger, TeamMailboxUnreadHintWorkerSettings,
};
use crate::team::TeamManager;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IdleUnreadHintState {
    pub(crate) session_id: String,
    pub(crate) idle_anchor_ts: i64,
    pub(crate) unread_count: i64,
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
            let delivery_id = format!(
                "team-mailbox-summary:{}:{}:{}:{}:{}",
                record.run_id,
                record.actor_id,
                runtime.session_id,
                action.idle_anchor_ts,
                action.unread_count
            );
            match self
                .agent_nudger
                .nudge_mailbox_prompt(
                    record.actor_id.as_str(),
                    Some(runtime.session_id.as_str()),
                    &delivery_id,
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

fn idle_unread_hint_key(run_id: &str, actor_id: &str) -> String {
    format!("{run_id}:{actor_id}")
}

pub(crate) fn actor_mailbox_is_idle(now: i64, idle_after_secs: i64, idle_anchor_ts: i64) -> bool {
    now.saturating_sub(idle_anchor_ts) >= idle_after_secs.max(1)
}

pub(crate) fn decide_idle_unread_hint_action(
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
