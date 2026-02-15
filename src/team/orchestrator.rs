use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::acp::AcpActorSkillContext;
use crate::agent::AgentManager;

use super::{TeamManager, TeamStepRecord, TeamStepStatus};

#[derive(Debug, Clone, Copy)]
pub struct TeamOrchestratorWorkerSettings {
    pub poll_interval_secs: i64,
    pub max_dispatch_per_tick: i64,
}

impl Default for TeamOrchestratorWorkerSettings {
    fn default() -> Self {
        Self {
            poll_interval_secs: 2,
            max_dispatch_per_tick: 32,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TeamOrchestratorDispatchSummary {
    pub scanned: i64,
    pub dispatched: i64,
    pub failed: i64,
}

#[derive(Clone)]
pub struct TeamOrchestratorWorker {
    teams: Arc<TeamManager>,
    agents: Arc<AgentManager>,
}

impl TeamOrchestratorWorker {
    pub fn new(teams: Arc<TeamManager>, agents: Arc<AgentManager>) -> Self {
        Self { teams, agents }
    }

    pub fn spawn(self, settings: TeamOrchestratorWorkerSettings) {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(
                settings.poll_interval_secs.max(1) as u64,
            ));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                match self.dispatch_once(settings.max_dispatch_per_tick).await {
                    Ok(summary) => {
                        if summary.scanned > 0 || summary.failed > 0 {
                            tracing::debug!(
                                scanned = summary.scanned,
                                dispatched = summary.dispatched,
                                failed = summary.failed,
                                "team orchestrator tick"
                            );
                        }
                    }
                    Err(err) => {
                        tracing::warn!("team orchestrator tick failed: {}", err);
                    }
                }
            }
        });
    }

    pub async fn dispatch_once(
        &self,
        max_dispatch_per_tick: i64,
    ) -> anyhow::Result<TeamOrchestratorDispatchSummary> {
        let max_dispatch = max_dispatch_per_tick.max(1) as usize;
        let runs = self
            .teams
            .list_active_runs(max_dispatch_per_tick.max(64))
            .await?;
        let mut summary = TeamOrchestratorDispatchSummary::default();

        for run in runs {
            if summary.dispatched as usize >= max_dispatch {
                break;
            }
            let steps = self.teams.list_steps(&run.id).await?;
            if steps.is_empty() {
                continue;
            }
            let status_by_key: HashMap<String, TeamStepStatus> = steps
                .iter()
                .map(|step| (step.step_key.clone(), step.status.clone()))
                .collect();

            for step in steps {
                if summary.dispatched as usize >= max_dispatch {
                    break;
                }
                if step.status != TeamStepStatus::Submitted {
                    continue;
                }
                summary.scanned += 1;
                if !is_step_ready(&step, &status_by_key) {
                    continue;
                }
                match self.dispatch_step(&run.id, &step).await {
                    Ok(()) => {
                        summary.dispatched += 1;
                    }
                    Err(err) => {
                        summary.failed += 1;
                        tracing::warn!(
                            run_id = %run.id,
                            step_id = %step.id,
                            step_key = %step.step_key,
                            member_id = %step.member_id,
                            "team orchestrator dispatch failed: {}",
                            err
                        );
                    }
                }
            }
        }
        Ok(summary)
    }

    async fn dispatch_step(&self, run_id: &str, step: &TeamStepRecord) -> anyhow::Result<()> {
        let actor_context = AcpActorSkillContext {
            run_id: run_id.to_string(),
            actor_id: step.member_id.clone(),
            default_channel: "default".to_string(),
            actor_cli_path: default_actor_cli_path(),
        };
        let start_result = self
            .agents
            .start_agent_with_actor_context(&step.member_id, Some(actor_context))
            .await;
        let session_id = match start_result {
            Ok(session_id) => session_id,
            Err(err) => {
                let err_text = format!(
                    "orchestrator failed to start member agent '{}' for step '{}': {}",
                    step.member_id, step.step_key, err
                );
                if let Err(fail_err) = self.teams.fail_step(&step.id, &err_text).await {
                    tracing::warn!(
                        run_id = %run_id,
                        step_id = %step.id,
                        "team orchestrator failed to mark step as failed: {}",
                        fail_err
                    );
                }
                return Err(err.context(err_text));
            }
        };
        let started = self.teams.start_step(&step.id, Some(&session_id)).await?;
        if started.status != TeamStepStatus::Working {
            tracing::debug!(
                run_id = %run_id,
                step_id = %started.id,
                status = ?started.status,
                "team orchestrator skip non-working start result"
            );
        }
        Ok(())
    }
}

fn default_actor_cli_path() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.into_os_string().into_string().ok())
        .unwrap_or_else(|| "agenthub".to_string())
}

fn is_step_ready(step: &TeamStepRecord, status_by_key: &HashMap<String, TeamStepStatus>) -> bool {
    step.depends_on.iter().all(|dep| {
        status_by_key
            .get(dep)
            .is_some_and(|status| *status == TeamStepStatus::Completed)
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use super::is_step_ready;
    use crate::team::{TeamStepRecord, TeamStepStatus};

    fn make_step(step_key: &str, depends_on: &[&str]) -> TeamStepRecord {
        TeamStepRecord {
            id: format!("step-{step_key}"),
            run_id: "run-1".to_string(),
            step_key: step_key.to_string(),
            member_id: "planner".to_string(),
            remote_task_id: None,
            status: TeamStepStatus::Submitted,
            attempt: 0,
            depends_on: depends_on.iter().map(|item| item.to_string()).collect(),
            input: Some(json!({"goal":"x"})),
            output: None,
            error_text: None,
            started_at: None,
            ended_at: None,
        }
    }

    #[test]
    fn step_is_ready_when_all_dependencies_completed() {
        let step = make_step("b", &["a", "c"]);
        let mut status_by_key = HashMap::new();
        status_by_key.insert("a".to_string(), TeamStepStatus::Completed);
        status_by_key.insert("c".to_string(), TeamStepStatus::Completed);
        assert!(is_step_ready(&step, &status_by_key));
    }

    #[test]
    fn step_is_not_ready_when_any_dependency_is_not_completed() {
        let step = make_step("b", &["a", "c"]);
        let mut status_by_key = HashMap::new();
        status_by_key.insert("a".to_string(), TeamStepStatus::Completed);
        status_by_key.insert("c".to_string(), TeamStepStatus::Working);
        assert!(!is_step_ready(&step, &status_by_key));
    }

    #[test]
    fn step_is_not_ready_when_dependency_is_missing() {
        let step = make_step("b", &["missing"]);
        let status_by_key = HashMap::new();
        assert!(!is_step_ready(&step, &status_by_key));
    }
}
