use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use async_trait::async_trait;
use serde_json::Value;

use crate::acp::{
    AcpActorContinuityEnvelope, AcpActorSkillContext, DEFAULT_ACTOR_CHANNEL, default_actor_cli_path,
};
use crate::agent::AgentManager;

use super::{TeamManager, TeamRunRecord, TeamStepRecord, TeamStepStatus};

#[derive(Debug, Clone, Copy)]
pub struct TeamOrchestratorWorkerSettings {
    pub poll_interval_secs: i64,
    pub max_dispatch_per_tick: i64,
    pub heartbeat_interval_ticks: i64,
}

impl Default for TeamOrchestratorWorkerSettings {
    fn default() -> Self {
        Self {
            poll_interval_secs: 2,
            max_dispatch_per_tick: 32,
            heartbeat_interval_ticks: 150,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TeamOrchestratorDispatchSummary {
    pub scanned: i64,
    pub dispatched: i64,
    pub failed: i64,
    pub reconciled_completed: i64,
    pub reconciled_failed: i64,
}

#[derive(Debug, Clone)]
struct OrchestratorStepSpec {
    step_key: String,
    member_id: String,
    depends_on: Vec<String>,
}

#[async_trait]
pub trait TeamMemberAgentStarter: Send + Sync {
    async fn start_member_agent(
        &self,
        member_id: &str,
        actor_context: AcpActorSkillContext,
    ) -> anyhow::Result<String>;

    async fn stop_member_agent(&self, member_id: &str) -> anyhow::Result<()>;
}

#[async_trait]
impl TeamMemberAgentStarter for AgentManager {
    async fn start_member_agent(
        &self,
        member_id: &str,
        actor_context: AcpActorSkillContext,
    ) -> anyhow::Result<String> {
        self.start_agent_with_actor_context(member_id, Some(actor_context))
            .await
    }

    async fn stop_member_agent(&self, member_id: &str) -> anyhow::Result<()> {
        self.stop_agent(member_id).await
    }
}

#[derive(Clone)]
pub struct TeamOrchestratorWorker {
    teams: Arc<TeamManager>,
    agent_starter: Arc<dyn TeamMemberAgentStarter>,
}

impl TeamOrchestratorWorker {
    pub fn new(teams: Arc<TeamManager>, agents: Arc<AgentManager>) -> Self {
        Self::with_agent_starter(teams, agents)
    }

    pub fn with_agent_starter(
        teams: Arc<TeamManager>,
        agent_starter: Arc<dyn TeamMemberAgentStarter>,
    ) -> Self {
        Self {
            teams,
            agent_starter,
        }
    }

    pub fn spawn(self, settings: TeamOrchestratorWorkerSettings) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let heartbeat_interval_ticks = settings.heartbeat_interval_ticks.max(1);
            let mut idle_ticks = 0_i64;
            let mut ticker = tokio::time::interval(Duration::from_secs(
                settings.poll_interval_secs.max(1) as u64,
            ));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                match self.dispatch_once(settings.max_dispatch_per_tick).await {
                    Ok(summary) => {
                        let has_activity = summary.scanned > 0
                            || summary.dispatched > 0
                            || summary.failed > 0
                            || summary.reconciled_completed > 0
                            || summary.reconciled_failed > 0;
                        if has_activity {
                            idle_ticks = 0;
                            tracing::debug!(
                                scanned = summary.scanned,
                                dispatched = summary.dispatched,
                                failed = summary.failed,
                                reconciled_completed = summary.reconciled_completed,
                                reconciled_failed = summary.reconciled_failed,
                                "team orchestrator tick"
                            );
                        } else {
                            idle_ticks += 1;
                            if idle_ticks % heartbeat_interval_ticks == 0 {
                                tracing::info!(
                                    idle_ticks,
                                    poll_interval_secs = settings.poll_interval_secs.max(1),
                                    max_dispatch_per_tick = settings.max_dispatch_per_tick.max(1),
                                    "team orchestrator heartbeat"
                                );
                            }
                        }
                    }
                    Err(err) => {
                        tracing::warn!("team orchestrator tick failed: {}", err);
                    }
                }
            }
        })
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
            self.bootstrap_run_steps_if_needed(&run).await?;
            let mut steps = self.teams.list_steps(&run.id).await?;
            if steps.is_empty() {
                continue;
            }
            let reconciled = self
                .reconcile_working_steps(&run.id, &steps, &mut summary)
                .await?;
            if reconciled {
                steps = self.teams.list_steps(&run.id).await?;
                if steps.is_empty() {
                    continue;
                }
            }

            loop {
                if summary.dispatched as usize >= max_dispatch {
                    break;
                }
                let status_by_key: HashMap<String, TeamStepStatus> = steps
                    .iter()
                    .map(|step| (step.step_key.clone(), step.status.clone()))
                    .collect();
                let mut dispatched_in_pass = false;
                let mut failed_in_pass = false;

                for step in &steps {
                    if summary.dispatched as usize >= max_dispatch {
                        break;
                    }
                    if step.status != TeamStepStatus::Submitted {
                        continue;
                    }
                    summary.scanned += 1;
                    if !is_step_ready(step, &status_by_key) {
                        continue;
                    }
                    match self.dispatch_step(&run.id, step).await {
                        Ok(()) => {
                            summary.dispatched += 1;
                            dispatched_in_pass = true;
                            break;
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
                            failed_in_pass = true;
                            break;
                        }
                    }
                }

                if failed_in_pass {
                    let _latest_steps = self.teams.list_steps(&run.id).await?;
                    break;
                }
                if !dispatched_in_pass {
                    break;
                }
                steps = self.teams.list_steps(&run.id).await?;
                if steps.is_empty() {
                    break;
                }
            }
        }
        Ok(summary)
    }

    async fn reconcile_working_steps(
        &self,
        run_id: &str,
        steps: &[TeamStepRecord],
        summary: &mut TeamOrchestratorDispatchSummary,
    ) -> anyhow::Result<bool> {
        let mut changed = false;
        for step in steps {
            if !matches!(
                step.status,
                TeamStepStatus::Working | TeamStepStatus::InputRequired
            ) {
                continue;
            }
            let Some(session_id) = step.remote_task_id.as_deref() else {
                continue;
            };
            let Some(session_status) = self.teams.get_agent_session_status(session_id).await?
            else {
                continue;
            };

            if session_status == "completed" {
                let completed = self.teams.complete_step(&step.id, None).await?;
                if completed.status == TeamStepStatus::Completed {
                    summary.reconciled_completed += 1;
                    changed = true;
                    tracing::debug!(
                        run_id = %run_id,
                        step_id = %completed.id,
                        step_key = %completed.step_key,
                        session_id = %session_id,
                        "team orchestrator reconciled completed step from agent session status"
                    );
                }
                continue;
            }

            if is_terminal_failure_session_status(&session_status) {
                let err_text = format!(
                    "orchestrator observed terminal agent session '{}' with status '{}'",
                    session_id, session_status
                );
                let failed = self.teams.fail_step(&step.id, &err_text).await?;
                if failed.status == TeamStepStatus::Failed {
                    summary.reconciled_failed += 1;
                    changed = true;
                    tracing::debug!(
                        run_id = %run_id,
                        step_id = %failed.id,
                        step_key = %failed.step_key,
                        session_id = %session_id,
                        session_status = %session_status,
                        "team orchestrator reconciled failed step from agent session status"
                    );
                }
            }
        }
        Ok(changed)
    }

    async fn bootstrap_run_steps_if_needed(&self, run: &TeamRunRecord) -> anyhow::Result<()> {
        let existing_steps = self.teams.list_steps(&run.id).await?;
        if !existing_steps.is_empty() {
            return Ok(());
        }
        let team = self.teams.get_team(&run.team_id).await?;
        let step_specs = parse_step_specs(&team.spec)?;
        for step_spec in step_specs {
            self.teams
                .submit_step(
                    &run.id,
                    &step_spec.step_key,
                    &step_spec.member_id,
                    step_spec.depends_on,
                    None,
                )
                .await
                .with_context(|| {
                    format!(
                        "failed to bootstrap step '{}' for run '{}'",
                        step_spec.step_key, run.id
                    )
                })?;
        }
        Ok(())
    }

    async fn dispatch_step(&self, run_id: &str, step: &TeamStepRecord) -> anyhow::Result<()> {
        let run = self.teams.get_run(run_id).await?;
        let continuity_mode = parse_run_continuity_mode(&run.input);
        let continuity_max_chars = parse_run_continuity_max_chars(&run.input);
        let member_role = self
            .resolve_member_role_for_step(&run.team_id, &step.member_id)
            .await
            .unwrap_or_else(|err| {
                tracing::warn!(
                    run_id = %run_id,
                    member_id = %step.member_id,
                    "team orchestrator failed to resolve member role: {}",
                    err
                );
                None
            });
        let continuity = if continuity_mode == "reset" {
            self.emit_continuity_event(
                run_id,
                "continuity_reset",
                serde_json::json!({
                    "step_id": step.id,
                    "step_key": step.step_key,
                    "member_id": step.member_id,
                    "mode": continuity_mode,
                }),
            )
            .await;
            None
        } else {
            match self
                .teams
                .get_member_continuity_state(&run.team_id, &step.member_id)
                .await
            {
                Ok(Some(state)) => {
                    self.emit_continuity_event(
                        run_id,
                        "continuity_attached",
                        serde_json::json!({
                            "step_id": step.id,
                            "step_key": step.step_key,
                            "member_id": step.member_id,
                            "mode": continuity_mode,
                            "source_run_id": state.source_run_id,
                            "source_session_id": state.source_session_id,
                        }),
                    )
                    .await;
                    Some(AcpActorContinuityEnvelope {
                        mode: continuity_mode.to_string(),
                        source_run_id: state.source_run_id,
                        source_session_id: state.source_session_id,
                        summary_text: truncate_text(
                            state.summary_text.as_str(),
                            continuity_max_chars,
                        ),
                        history_window: state.history_window,
                    })
                }
                Ok(None) => {
                    self.emit_continuity_event(
                        run_id,
                        "continuity_fallback",
                        serde_json::json!({
                            "step_id": step.id,
                            "step_key": step.step_key,
                            "member_id": step.member_id,
                            "mode": continuity_mode,
                            "reason": "missing_state",
                        }),
                    )
                    .await;
                    None
                }
                Err(err) => {
                    tracing::warn!(
                        run_id = %run_id,
                        step_id = %step.id,
                        member_id = %step.member_id,
                        "team orchestrator continuity lookup failed: {}",
                        err
                    );
                    self.emit_continuity_event(
                        run_id,
                        "continuity_fallback",
                        serde_json::json!({
                            "step_id": step.id,
                            "step_key": step.step_key,
                            "member_id": step.member_id,
                            "mode": continuity_mode,
                            "reason": "state_lookup_failed",
                        }),
                    )
                    .await;
                    None
                }
            }
        };
        let actor_context = AcpActorSkillContext {
            run_id: run_id.to_string(),
            actor_id: step.member_id.clone(),
            default_channel: DEFAULT_ACTOR_CHANNEL.to_string(),
            actor_cli_path: default_actor_cli_path()?,
            member_role,
            continuity,
        };
        let start_result = self
            .agent_starter
            .start_member_agent(&step.member_id, actor_context)
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
        let started = match self.teams.start_step(&step.id, Some(&session_id)).await {
            Ok(started) => started,
            Err(err) => {
                if let Err(stop_err) = self.agent_starter.stop_member_agent(&step.member_id).await {
                    tracing::warn!(
                        run_id = %run_id,
                        step_id = %step.id,
                        member_id = %step.member_id,
                        "team orchestrator failed to stop member agent after start_step error: {}",
                        stop_err
                    );
                }
                return Err(err.context(format!(
                    "failed to start step '{}' for run '{}'",
                    step.step_key, run_id
                )));
            }
        };
        if started.status != TeamStepStatus::Working {
            if let Err(err) = self.agent_starter.stop_member_agent(&step.member_id).await {
                tracing::warn!(
                    run_id = %run_id,
                    step_id = %started.id,
                    member_id = %step.member_id,
                    "team orchestrator failed to stop member agent after non-working step start: {}",
                    err
                );
            }
            tracing::warn!(
                run_id = %run_id,
                step_id = %started.id,
                status = ?started.status,
                "team orchestrator skip non-working start result"
            );
            return Err(anyhow::anyhow!(
                "step '{}' transitioned to '{:?}' while dispatching",
                started.step_key,
                started.status
            ));
        }
        Ok(())
    }

    async fn emit_continuity_event(&self, run_id: &str, event_type: &str, payload: Value) {
        if let Err(err) = self
            .teams
            .append_run_event(run_id, event_type, payload)
            .await
        {
            tracing::warn!(
                run_id = %run_id,
                event_type = %event_type,
                "team orchestrator failed to append continuity event: {}",
                err
            );
        }
    }

    async fn resolve_member_role_for_step(
        &self,
        team_id: &str,
        member_id: &str,
    ) -> anyhow::Result<Option<String>> {
        let team = self.teams.get_team(team_id).await?;
        Ok(parse_member_role(&team.spec, member_id))
    }
}

fn is_step_ready(step: &TeamStepRecord, status_by_key: &HashMap<String, TeamStepStatus>) -> bool {
    step.depends_on.iter().all(|dep| {
        status_by_key
            .get(dep)
            .is_some_and(|status| matches!(status, TeamStepStatus::Completed))
    })
}

fn is_terminal_failure_session_status(status: &str) -> bool {
    matches!(status, "failed" | "cancelled" | "exited")
}

fn parse_run_continuity_mode(run_input: &Value) -> &'static str {
    let Some(run_obj) = run_input.as_object() else {
        return "inherit_recent";
    };
    let mode = run_obj
        .get("continuity")
        .and_then(Value::as_object)
        .and_then(|continuity| continuity.get("mode"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("inherit_recent");
    match mode {
        "reset" => "reset",
        _ => "inherit_recent",
    }
}

fn parse_run_continuity_max_chars(run_input: &Value) -> usize {
    let Some(run_obj) = run_input.as_object() else {
        return 1200;
    };
    let Some(raw) = run_obj
        .get("continuity")
        .and_then(Value::as_object)
        .and_then(|continuity| continuity.get("max_chars"))
        .and_then(Value::as_i64)
    else {
        return 1200;
    };
    raw.clamp(256, 20000) as usize
}

fn truncate_text(raw: &str, max_chars: usize) -> String {
    if raw.is_empty() || max_chars == 0 {
        return String::new();
    }
    raw.chars().take(max_chars).collect::<String>()
}

fn parse_step_specs(spec: &Value) -> anyhow::Result<Vec<OrchestratorStepSpec>> {
    let spec_obj = spec
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("team spec must be an object"))?;
    let entrypoint = spec_obj
        .get("entrypoint")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("spec.entrypoint is required"))?;

    if let Some(steps) = spec_obj.get("steps").and_then(Value::as_array) {
        if steps.is_empty() {
            return Err(anyhow::anyhow!("spec.steps must not be empty"));
        }
        let mut out = Vec::with_capacity(steps.len());
        let mut step_keys = HashSet::with_capacity(steps.len());
        for step in steps {
            let step = step
                .as_object()
                .ok_or_else(|| anyhow::anyhow!("spec.steps entries must be objects"))?;
            let step_key = step
                .get("step_key")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow::anyhow!("spec.steps[].step_key is required"))?
                .to_string();
            if !step_keys.insert(step_key.clone()) {
                return Err(anyhow::anyhow!(
                    "spec.steps contains duplicated step_key '{}'",
                    step_key
                ));
            }
            let member_id = step
                .get("member_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow::anyhow!("spec.steps[].member_id is required"))?
                .to_string();
            let depends_on = step
                .get("depends_on")
                .and_then(Value::as_array)
                .map(|deps| {
                    deps.iter()
                        .filter_map(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            out.push(OrchestratorStepSpec {
                step_key,
                member_id,
                depends_on,
            });
        }
        if !step_keys.contains(entrypoint) {
            return Err(anyhow::anyhow!(
                "spec.entrypoint '{}' must exist in spec.steps[].step_key",
                entrypoint
            ));
        }
        for step in &out {
            for dep in &step.depends_on {
                if !step_keys.contains(dep) {
                    return Err(anyhow::anyhow!(
                        "spec.steps dependency '{}' referenced by '{}' does not exist",
                        dep,
                        step.step_key
                    ));
                }
            }
        }
        ensure_acyclic_step_specs(&out)?;
        return Ok(out);
    }

    Ok(vec![OrchestratorStepSpec {
        step_key: entrypoint.to_string(),
        member_id: entrypoint.to_string(),
        depends_on: Vec::new(),
    }])
}

fn parse_member_role(spec: &Value, member_id: &str) -> Option<String> {
    let normalized_member_id = member_id.trim();
    if normalized_member_id.is_empty() {
        return None;
    }
    spec.as_object()
        .and_then(|spec_obj| spec_obj.get("members"))
        .and_then(Value::as_array)
        .and_then(|members| {
            members.iter().find_map(|member| {
                let member = member.as_object()?;
                let id = member
                    .get("member_id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())?;
                if id != normalized_member_id {
                    return None;
                }
                member
                    .get("role")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_ascii_lowercase)
            })
        })
}

fn ensure_acyclic_step_specs(step_specs: &[OrchestratorStepSpec]) -> anyhow::Result<()> {
    let by_key: HashMap<&str, &OrchestratorStepSpec> = step_specs
        .iter()
        .map(|step| (step.step_key.as_str(), step))
        .collect();
    let mut permanent = HashSet::with_capacity(step_specs.len());
    let mut temporary = HashSet::with_capacity(step_specs.len());
    let mut stack = Vec::new();
    for step in step_specs {
        if permanent.contains(step.step_key.as_str()) {
            continue;
        }
        detect_cycle_dfs(
            step.step_key.as_str(),
            &by_key,
            &mut permanent,
            &mut temporary,
            &mut stack,
        )?;
    }
    Ok(())
}

fn detect_cycle_dfs<'a>(
    current: &'a str,
    by_key: &HashMap<&'a str, &'a OrchestratorStepSpec>,
    permanent: &mut HashSet<&'a str>,
    temporary: &mut HashSet<&'a str>,
    stack: &mut Vec<&'a str>,
) -> anyhow::Result<()> {
    if permanent.contains(current) {
        return Ok(());
    }
    if !temporary.insert(current) {
        let start = stack.iter().position(|item| *item == current).unwrap_or(0);
        let mut cycle = stack[start..].to_vec();
        cycle.push(current);
        return Err(anyhow::anyhow!(
            "spec.steps contains dependency cycle: {}",
            cycle.join(" -> ")
        ));
    }

    stack.push(current);
    let step = by_key
        .get(current)
        .copied()
        .ok_or_else(|| anyhow::anyhow!("missing step spec for '{}'", current))?;
    for dep in &step.depends_on {
        detect_cycle_dfs(dep.as_str(), by_key, permanent, temporary, stack)?;
    }
    stack.pop();

    temporary.remove(current);
    permanent.insert(current);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use serde_json::json;
    use sqlx::SqlitePool;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    use super::{
        TeamMemberAgentStarter, TeamOrchestratorWorker, TeamOrchestratorWorkerSettings,
        is_step_ready, is_terminal_failure_session_status, parse_member_role, parse_step_specs,
    };
    use crate::acp::AcpActorSkillContext;
    use crate::team::{
        SendActorMessageInput, TeamActorMessageStatus, TeamActorMessageTransport,
        TeamDefinitionConfig, TeamManager, TeamStepRecord, TeamStepStatus,
    };

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

    #[test]
    fn parse_step_specs_defaults_to_entrypoint_member_when_steps_omitted() {
        let specs = parse_step_specs(&json!({
            "entrypoint":"planner",
            "members":[{"member_id":"planner"}]
        }))
        .expect("parse step specs");
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].step_key, "planner");
        assert_eq!(specs[0].member_id, "planner");
        assert!(specs[0].depends_on.is_empty());
    }

    #[test]
    fn parse_step_specs_rejects_missing_dependency() {
        let err = parse_step_specs(&json!({
            "entrypoint":"step_plan",
            "steps":[
                {"step_key":"step_plan","member_id":"planner","depends_on":["missing_step"]}
            ]
        }))
        .expect_err("missing dependency should be rejected");
        let message = err.to_string();
        assert!(
            message.contains("does not exist"),
            "unexpected error: {message}"
        );
    }

    #[test]
    fn parse_step_specs_rejects_dependency_cycle() {
        let err = parse_step_specs(&json!({
            "entrypoint":"a",
            "steps":[
                {"step_key":"a","member_id":"planner","depends_on":["c"]},
                {"step_key":"b","member_id":"reviewer","depends_on":["a"]},
                {"step_key":"c","member_id":"writer","depends_on":["b"]}
            ]
        }))
        .expect_err("cyclic dependency should be rejected");
        let message = err.to_string();
        assert!(message.contains("cycle"), "unexpected error: {message}");
    }

    #[test]
    fn parse_step_specs_rejects_entrypoint_not_in_steps() {
        let err = parse_step_specs(&json!({
            "entrypoint":"not_present",
            "steps":[
                {"step_key":"step_plan","member_id":"planner"}
            ]
        }))
        .expect_err("entrypoint should be part of steps");
        let message = err.to_string();
        assert!(
            message.contains("spec.entrypoint"),
            "unexpected error: {message}"
        );
    }

    #[test]
    fn parse_member_role_returns_expected_role() {
        let role = parse_member_role(
            &json!({
                "members":[
                    {"member_id":"planner","role":"leader"},
                    {"member_id":"reviewer","role":"worker"}
                ]
            }),
            "planner",
        );
        assert_eq!(role.as_deref(), Some("leader"));
    }

    #[test]
    fn parse_member_role_returns_none_for_missing_member_or_role() {
        assert!(
            parse_member_role(
                &json!({
                    "members":[{"member_id":"reviewer","role":"worker"}]
                }),
                "planner",
            )
            .is_none()
        );
        assert!(
            parse_member_role(
                &json!({
                    "members":[{"member_id":"planner"}]
                }),
                "planner",
            )
            .is_none()
        );
    }

    #[test]
    fn terminal_failure_session_status_detection_matches_expected_values() {
        assert!(is_terminal_failure_session_status("failed"));
        assert!(is_terminal_failure_session_status("cancelled"));
        assert!(is_terminal_failure_session_status("exited"));
        assert!(!is_terminal_failure_session_status("running"));
        assert!(!is_terminal_failure_session_status("completed"));
    }

    #[derive(Debug, Clone)]
    struct FakeStartCall {
        member_id: String,
        actor_context: AcpActorSkillContext,
    }

    #[derive(Clone, Default)]
    struct FakeAgentStarter {
        calls: Arc<Mutex<Vec<FakeStartCall>>>,
        stop_calls: Arc<Mutex<Vec<String>>>,
        fail_members: Arc<Mutex<HashSet<String>>>,
    }

    impl FakeAgentStarter {
        fn mark_fail_for(&self, member_id: &str) {
            self.fail_members
                .lock()
                .expect("lock fail_members")
                .insert(member_id.to_string());
        }

        fn calls(&self) -> Vec<FakeStartCall> {
            self.calls.lock().expect("lock calls").clone()
        }

        fn stop_calls(&self) -> Vec<String> {
            self.stop_calls.lock().expect("lock stop_calls").clone()
        }
    }

    #[async_trait]
    impl TeamMemberAgentStarter for FakeAgentStarter {
        async fn start_member_agent(
            &self,
            member_id: &str,
            actor_context: AcpActorSkillContext,
        ) -> anyhow::Result<String> {
            self.calls.lock().expect("lock calls").push(FakeStartCall {
                member_id: member_id.to_string(),
                actor_context,
            });
            if self
                .fail_members
                .lock()
                .expect("lock fail_members")
                .contains(member_id)
            {
                return Err(anyhow::anyhow!("forced starter failure for {}", member_id));
            }
            Ok(format!("session-{member_id}"))
        }

        async fn stop_member_agent(&self, member_id: &str) -> anyhow::Result<()> {
            self.stop_calls
                .lock()
                .expect("lock stop_calls")
                .push(member_id.to_string());
            Ok(())
        }
    }

    async fn setup_test_db() -> SqlitePool {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect sqlite");

        sqlx::query(
            r#"
            CREATE TABLE team_definitions (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                description TEXT,
                spec_json TEXT NOT NULL,
                owner_user_id TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("create team_definitions");

        sqlx::query(
            r#"
            CREATE TABLE team_runs (
                id TEXT PRIMARY KEY,
                team_id TEXT NOT NULL,
                context_id TEXT NOT NULL,
                status TEXT NOT NULL,
                input_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                started_at INTEGER,
                ended_at INTEGER,
                FOREIGN KEY(team_id) REFERENCES team_definitions(id)
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("create team_runs");

        sqlx::query(
            r#"
            CREATE TABLE team_steps (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                step_key TEXT NOT NULL,
                member_id TEXT NOT NULL,
                remote_task_id TEXT,
                status TEXT NOT NULL,
                attempt INTEGER NOT NULL DEFAULT 0,
                depends_on_json TEXT NOT NULL DEFAULT '[]',
                input_json TEXT,
                output_json TEXT,
                error_text TEXT,
                started_at INTEGER,
                ended_at INTEGER,
                UNIQUE(run_id, step_key, attempt),
                FOREIGN KEY(run_id) REFERENCES team_runs(id)
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("create team_steps");

        sqlx::query(
            r#"
            CREATE TABLE team_run_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id TEXT NOT NULL,
                step_id TEXT,
                event_type TEXT NOT NULL,
                ts INTEGER NOT NULL,
                payload_json TEXT NOT NULL,
                FOREIGN KEY(run_id) REFERENCES team_runs(id)
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("create team_run_events");

        sqlx::query(
            r#"
            CREATE TABLE team_actor_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id TEXT NOT NULL,
                from_actor_id TEXT NOT NULL,
                to_actor_id TEXT NOT NULL,
                channel TEXT NOT NULL,
                transport TEXT NOT NULL,
                route_json TEXT,
                payload_json TEXT NOT NULL,
                idempotency_key TEXT,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                delivered_at INTEGER,
                relay_attempt INTEGER NOT NULL DEFAULT 0,
                relay_next_retry_at INTEGER,
                relay_last_error TEXT,
                dead_letter_at INTEGER,
                FOREIGN KEY(run_id) REFERENCES team_runs(id)
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("create team_actor_messages");

        sqlx::query(
            r#"
            CREATE TABLE team_member_continuity_state (
                team_id TEXT NOT NULL,
                member_id TEXT NOT NULL,
                source_run_id TEXT NOT NULL,
                source_session_id TEXT,
                summary_text TEXT NOT NULL,
                history_window_json TEXT NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (team_id, member_id),
                FOREIGN KEY(team_id) REFERENCES team_definitions(id),
                FOREIGN KEY(source_run_id) REFERENCES team_runs(id)
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("create team_member_continuity_state");

        sqlx::query(
            r#"
            CREATE TABLE team_context_artifacts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                team_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                member_id TEXT NOT NULL,
                session_id TEXT,
                artifact_seq INTEGER NOT NULL,
                artifact_kind TEXT NOT NULL,
                artifact_path TEXT NOT NULL,
                artifact_size_bytes INTEGER NOT NULL,
                content_checksum TEXT,
                created_at INTEGER NOT NULL,
                FOREIGN KEY(team_id) REFERENCES team_definitions(id),
                FOREIGN KEY(run_id) REFERENCES team_runs(id)
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("create team_context_artifacts");

        sqlx::query(
            r#"
            CREATE UNIQUE INDEX idx_team_actor_messages_idempotency
            ON team_actor_messages(run_id, from_actor_id, idempotency_key)
            WHERE idempotency_key IS NOT NULL
            "#,
        )
        .execute(&pool)
        .await
        .expect("create team_actor_messages idempotency index");

        sqlx::query(
            r#"
            CREATE TABLE agents (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                workdir TEXT NOT NULL,
                command TEXT NOT NULL,
                args TEXT NOT NULL,
                worktree_mode TEXT NOT NULL,
                worktree_repo TEXT,
                worktree_ref TEXT,
                code_mode INTEGER NOT NULL DEFAULT 0,
                source TEXT NOT NULL DEFAULT 'manual',
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("create agents");

        sqlx::query(
            r#"
            CREATE TABLE agent_sessions (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                status TEXT NOT NULL,
                started_at INTEGER NOT NULL,
                ended_at INTEGER
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("create agent_sessions");

        pool
    }

    #[tokio::test]
    async fn dispatch_once_injects_actor_runtime_and_supports_inbox_ack_flow() {
        let db = setup_test_db().await;
        let teams = Arc::new(TeamManager::new(db));
        let team = teams
            .create_team(TeamDefinitionConfig {
                name: "orchestrator-team".to_string(),
                description: Some("team for orchestrator dispatch test".to_string()),
                spec: json!({
                    "entrypoint":"planner",
                    "members":[
                        {"member_id":"planner","role":"leader"},
                        {"member_id":"reviewer","role":"worker"}
                    ]
                }),
            })
            .await
            .expect("create team");
        let run = teams
            .create_run(&team.id, Some("ctx-orchestrator"), json!({"prompt":"go"}))
            .await
            .expect("create run");
        let step = teams
            .submit_step(
                &run.id,
                "plan",
                "planner",
                Vec::new(),
                Some(json!({"goal":"plan"})),
            )
            .await
            .expect("submit step");

        let starter = Arc::new(FakeAgentStarter::default());
        let worker = TeamOrchestratorWorker::with_agent_starter(teams.clone(), starter.clone());
        let summary = worker.dispatch_once(10).await.expect("dispatch once");

        assert_eq!(summary.scanned, 1);
        assert_eq!(summary.dispatched, 1);
        assert_eq!(summary.failed, 0);

        let calls = starter.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].member_id, "planner");
        assert_eq!(calls[0].actor_context.run_id, run.id);
        assert_eq!(calls[0].actor_context.actor_id, "planner");
        assert_eq!(calls[0].actor_context.default_channel, "default");
        assert_eq!(
            calls[0].actor_context.member_role.as_deref(),
            Some("leader")
        );
        assert!(calls[0].actor_context.continuity.is_none());

        let started_step = teams.get_step(&step.id).await.expect("get started step");
        assert_eq!(started_step.status, TeamStepStatus::Working);
        assert_eq!(
            started_step.remote_task_id.as_deref(),
            Some("session-planner")
        );

        let sent = teams
            .send_actor_message(SendActorMessageInput {
                run_id: &run.id,
                from_actor_id: "reviewer",
                to_actor_id: &calls[0].actor_context.actor_id,
                channel: "default",
                transport: TeamActorMessageTransport::Local,
                route: None,
                payload: json!({"text":"ping"}),
                idempotency_key: Some("orchestrator-inbox-ack"),
            })
            .await
            .expect("send actor message");

        let inbox = teams
            .list_actor_inbox(&run.id, &calls[0].actor_context.actor_id, 10, None, false)
            .await
            .expect("list inbox");
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].message_id, sent.message_id);
        assert_eq!(inbox[0].status, TeamActorMessageStatus::Pending);

        let acked = teams
            .ack_actor_message(&run.id, &calls[0].actor_context.actor_id, sent.message_id)
            .await
            .expect("ack message");
        assert_eq!(acked.status, TeamActorMessageStatus::Delivered);
    }

    #[tokio::test]
    async fn dispatch_once_attaches_continuity_when_available() {
        let db = setup_test_db().await;
        let teams = Arc::new(TeamManager::new(db));
        let team = teams
            .create_team(TeamDefinitionConfig {
                name: "orchestrator-continuity-team".to_string(),
                description: Some("team for continuity attach test".to_string()),
                spec: json!({
                    "entrypoint":"planner",
                    "members":[
                        {"member_id":"planner","role":"leader"}
                    ]
                }),
            })
            .await
            .expect("create team");

        let previous_run = teams
            .create_run(&team.id, Some("ctx-prev"), json!({"prompt":"prev"}))
            .await
            .expect("create previous run");
        let previous_step = teams
            .submit_step(
                &previous_run.id,
                "plan_prev",
                "planner",
                Vec::new(),
                Some(json!({"goal":"seed continuity"})),
            )
            .await
            .expect("submit previous step");
        let _ = teams
            .start_step(&previous_step.id, Some("session-prev-planner"))
            .await
            .expect("start previous step");
        let _ = teams
            .complete_step(
                &previous_step.id,
                Some(json!({"summary":"previous synthesis for continuity"})),
            )
            .await
            .expect("complete previous step");

        let run = teams
            .create_run(
                &team.id,
                Some("ctx-next"),
                json!({"prompt":"next", "continuity": {"mode":"inherit_recent"}}),
            )
            .await
            .expect("create next run");
        let step = teams
            .submit_step(
                &run.id,
                "plan_next",
                "planner",
                Vec::new(),
                Some(json!({"goal":"use continuity"})),
            )
            .await
            .expect("submit next step");

        let starter = Arc::new(FakeAgentStarter::default());
        let worker = TeamOrchestratorWorker::with_agent_starter(teams.clone(), starter.clone());
        let summary = worker.dispatch_once(10).await.expect("dispatch once");
        assert_eq!(summary.dispatched, 1);

        let calls = starter.calls();
        assert_eq!(calls.len(), 1);
        let continuity = calls[0]
            .actor_context
            .continuity
            .as_ref()
            .expect("continuity should be attached");
        assert_eq!(continuity.mode, "inherit_recent");
        assert_eq!(continuity.source_run_id, previous_run.id);
        assert_eq!(
            continuity.source_session_id.as_deref(),
            Some("session-prev-planner")
        );
        assert!(continuity.summary_text.contains("previous synthesis"));

        let events = teams
            .list_run_events(&run.id, 100, None)
            .await
            .expect("list run events");
        assert!(
            events
                .iter()
                .any(|event| event.event_type == "continuity_attached"),
            "continuity_attached event should exist"
        );

        let started_step = teams.get_step(&step.id).await.expect("get started step");
        assert_eq!(started_step.status, TeamStepStatus::Working);
    }

    #[tokio::test]
    async fn dispatch_once_respects_continuity_reset_mode() {
        let db = setup_test_db().await;
        let teams = Arc::new(TeamManager::new(db));
        let team = teams
            .create_team(TeamDefinitionConfig {
                name: "orchestrator-continuity-reset-team".to_string(),
                description: Some("team for continuity reset test".to_string()),
                spec: json!({
                    "entrypoint":"planner",
                    "members":[
                        {"member_id":"planner","role":"leader"}
                    ]
                }),
            })
            .await
            .expect("create team");

        let previous_run = teams
            .create_run(&team.id, Some("ctx-prev-reset"), json!({"prompt":"prev"}))
            .await
            .expect("create previous run");
        let previous_step = teams
            .submit_step(
                &previous_run.id,
                "plan_prev_reset",
                "planner",
                Vec::new(),
                Some(json!({"goal":"seed continuity reset"})),
            )
            .await
            .expect("submit previous step");
        let _ = teams
            .start_step(&previous_step.id, Some("session-prev-reset"))
            .await
            .expect("start previous step");
        let _ = teams
            .complete_step(
                &previous_step.id,
                Some(json!({"summary":"should be ignored by reset mode"})),
            )
            .await
            .expect("complete previous step");

        let run = teams
            .create_run(
                &team.id,
                Some("ctx-next-reset"),
                json!({"prompt":"next", "continuity": {"mode":"reset"}}),
            )
            .await
            .expect("create next run");
        let _step = teams
            .submit_step(
                &run.id,
                "plan_next_reset",
                "planner",
                Vec::new(),
                Some(json!({"goal":"ignore continuity"})),
            )
            .await
            .expect("submit next step");

        let starter = Arc::new(FakeAgentStarter::default());
        let worker = TeamOrchestratorWorker::with_agent_starter(teams.clone(), starter.clone());
        let summary = worker.dispatch_once(10).await.expect("dispatch once");
        assert_eq!(summary.dispatched, 1);

        let calls = starter.calls();
        assert_eq!(calls.len(), 1);
        assert!(
            calls[0].actor_context.continuity.is_none(),
            "reset mode should not attach continuity"
        );

        let events = teams
            .list_run_events(&run.id, 100, None)
            .await
            .expect("list run events");
        assert!(
            events
                .iter()
                .any(|event| event.event_type == "continuity_reset"),
            "continuity_reset event should exist"
        );
    }

    #[tokio::test]
    async fn dispatch_once_marks_step_failed_when_member_start_fails() {
        let db = setup_test_db().await;
        let teams = Arc::new(TeamManager::new(db));
        let team = teams
            .create_team(TeamDefinitionConfig {
                name: "orchestrator-fail-team".to_string(),
                description: Some("team for orchestrator failure test".to_string()),
                spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
            })
            .await
            .expect("create team");
        let run = teams
            .create_run(&team.id, Some("ctx-fail"), json!({"prompt":"go"}))
            .await
            .expect("create run");
        let step = teams
            .submit_step(
                &run.id,
                "plan",
                "planner",
                Vec::new(),
                Some(json!({"goal":"plan"})),
            )
            .await
            .expect("submit step");

        let starter = Arc::new(FakeAgentStarter::default());
        starter.mark_fail_for("planner");

        let worker = TeamOrchestratorWorker::with_agent_starter(teams.clone(), starter);
        let summary = worker.dispatch_once(10).await.expect("dispatch once");

        assert_eq!(summary.scanned, 1);
        assert_eq!(summary.dispatched, 0);
        assert_eq!(summary.failed, 1);

        let failed_step = teams.get_step(&step.id).await.expect("get failed step");
        assert_eq!(failed_step.status, TeamStepStatus::Failed);
        assert!(
            failed_step
                .error_text
                .as_deref()
                .is_some_and(|text| text.contains("failed to start member agent"))
        );
    }

    #[tokio::test]
    async fn dispatch_once_stops_run_dispatch_after_first_failure_in_tick() {
        let db = setup_test_db().await;
        let teams = Arc::new(TeamManager::new(db));
        let team = teams
            .create_team(TeamDefinitionConfig {
                name: "orchestrator-stop-after-failure-team".to_string(),
                description: Some("team for failure short-circuit in single tick".to_string()),
                spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"},{"member_id":"reviewer"}]}),
            })
            .await
            .expect("create team");
        let run = teams
            .create_run(
                &team.id,
                Some("ctx-stop-after-failure"),
                json!({"prompt":"go"}),
            )
            .await
            .expect("create run");
        let failed_step = teams
            .submit_step(
                &run.id,
                "a_plan",
                "planner",
                Vec::new(),
                Some(json!({"goal":"plan"})),
            )
            .await
            .expect("submit failing step");
        let deferred_step = teams
            .submit_step(
                &run.id,
                "b_review",
                "reviewer",
                Vec::new(),
                Some(json!({"goal":"review"})),
            )
            .await
            .expect("submit deferred step");

        let starter = Arc::new(FakeAgentStarter::default());
        starter.mark_fail_for("planner");
        let worker = TeamOrchestratorWorker::with_agent_starter(teams.clone(), starter.clone());
        let summary = worker.dispatch_once(10).await.expect("dispatch once");

        assert_eq!(summary.failed, 1);
        assert_eq!(summary.dispatched, 0);
        let calls = starter.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].member_id, "planner");

        let failed_step_after = teams
            .get_step(&failed_step.id)
            .await
            .expect("get failed step");
        assert_eq!(failed_step_after.status, TeamStepStatus::Failed);
        let deferred_step_after = teams
            .get_step(&deferred_step.id)
            .await
            .expect("get deferred step");
        assert_eq!(deferred_step_after.status, TeamStepStatus::Submitted);
    }

    #[tokio::test]
    async fn dispatch_step_returns_error_and_stops_member_when_step_is_not_working() {
        let db = setup_test_db().await;
        let teams = Arc::new(TeamManager::new(db));
        let team = teams
            .create_team(TeamDefinitionConfig {
                name: "orchestrator-non-working-start-team".to_string(),
                description: Some("team for dispatch non-working start result".to_string()),
                spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
            })
            .await
            .expect("create team");
        let run = teams
            .create_run(&team.id, Some("ctx-non-working"), json!({"prompt":"go"}))
            .await
            .expect("create run");
        let step = teams
            .submit_step(
                &run.id,
                "plan",
                "planner",
                Vec::new(),
                Some(json!({"goal":"plan"})),
            )
            .await
            .expect("submit step");
        let _ = teams
            .fail_step(&step.id, "forced terminal state before dispatch")
            .await
            .expect("fail step before dispatch");

        let starter = Arc::new(FakeAgentStarter::default());
        let worker = TeamOrchestratorWorker::with_agent_starter(teams.clone(), starter.clone());
        let err = worker
            .dispatch_step(&run.id, &step)
            .await
            .expect_err("dispatch step should fail when start_step does not enter working");
        let message = err.to_string();
        assert!(
            message.contains("transitioned"),
            "unexpected dispatch error: {message}"
        );
        assert_eq!(starter.calls().len(), 1);
        assert_eq!(starter.stop_calls(), vec!["planner".to_string()]);
    }

    #[tokio::test]
    async fn dispatch_step_stops_member_when_start_step_returns_error() {
        let db = setup_test_db().await;
        let teams = Arc::new(TeamManager::new(db.clone()));
        let team = teams
            .create_team(TeamDefinitionConfig {
                name: "orchestrator-start-step-error-team".to_string(),
                description: Some("team for dispatch start_step error handling".to_string()),
                spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
            })
            .await
            .expect("create team");
        let run = teams
            .create_run(
                &team.id,
                Some("ctx-start-step-error"),
                json!({"prompt":"go"}),
            )
            .await
            .expect("create run");
        let step = teams
            .submit_step(
                &run.id,
                "plan",
                "planner",
                Vec::new(),
                Some(json!({"goal":"plan"})),
            )
            .await
            .expect("submit step");
        sqlx::query("DROP TABLE team_steps")
            .execute(&db)
            .await
            .expect("drop team_steps to force start_step error");

        let starter = Arc::new(FakeAgentStarter::default());
        let worker = TeamOrchestratorWorker::with_agent_starter(teams.clone(), starter.clone());
        let err = worker
            .dispatch_step(&run.id, &step)
            .await
            .expect_err("dispatch step should fail when start_step returns error");
        let message = err.to_string();
        assert!(
            message.contains("failed to start step"),
            "unexpected error: {message}"
        );
        assert_eq!(starter.calls().len(), 1);
        assert_eq!(starter.stop_calls(), vec!["planner".to_string()]);
    }

    #[tokio::test]
    async fn dispatch_once_bootstraps_spec_steps_and_respects_dependencies() {
        let db = setup_test_db().await;
        let teams = Arc::new(TeamManager::new(db));
        let team = teams
            .create_team(TeamDefinitionConfig {
                name: "orchestrator-bootstrap-team".to_string(),
                description: Some("team for orchestrator bootstrap flow".to_string()),
                spec: json!({
                    "entrypoint":"step_plan",
                    "members":[{"member_id":"planner"},{"member_id":"reviewer"}],
                    "steps":[
                        {"step_key":"step_plan","member_id":"planner"},
                        {"step_key":"step_review","member_id":"reviewer","depends_on":["step_plan"]}
                    ]
                }),
            })
            .await
            .expect("create team");
        let run = teams
            .create_run(&team.id, Some("ctx-bootstrap"), json!({"prompt":"go"}))
            .await
            .expect("create run");

        let starter = Arc::new(FakeAgentStarter::default());
        let worker = TeamOrchestratorWorker::with_agent_starter(teams.clone(), starter.clone());
        let first_summary = worker.dispatch_once(10).await.expect("first dispatch");
        assert_eq!(first_summary.dispatched, 1);

        let steps_after_first = teams.list_steps(&run.id).await.expect("list steps");
        assert_eq!(steps_after_first.len(), 2);
        let step_plan = steps_after_first
            .iter()
            .find(|step| step.step_key == "step_plan")
            .expect("find step_plan");
        let step_review = steps_after_first
            .iter()
            .find(|step| step.step_key == "step_review")
            .expect("find step_review");
        assert_eq!(step_plan.status, TeamStepStatus::Working);
        assert_eq!(step_review.status, TeamStepStatus::Submitted);

        let _ = teams
            .complete_step(&step_plan.id, Some(json!({"result":"planned"})))
            .await
            .expect("complete step_plan");

        let second_summary = worker.dispatch_once(10).await.expect("second dispatch");
        assert_eq!(second_summary.dispatched, 1);

        let steps_after_second = teams.list_steps(&run.id).await.expect("list steps");
        let step_review_after_second = steps_after_second
            .iter()
            .find(|step| step.step_key == "step_review")
            .expect("find step_review");
        assert_eq!(step_review_after_second.status, TeamStepStatus::Working);

        let calls = starter.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].member_id, "planner");
        assert_eq!(calls[1].member_id, "reviewer");
    }

    #[tokio::test]
    async fn dispatch_once_reconciles_working_step_from_completed_session() {
        let db = setup_test_db().await;
        let teams = Arc::new(TeamManager::new(db.clone()));
        let team = teams
            .create_team(TeamDefinitionConfig {
                name: "orchestrator-reconcile-complete-team".to_string(),
                description: Some("team for orchestrator reconcile completion".to_string()),
                spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
            })
            .await
            .expect("create team");
        let run = teams
            .create_run(
                &team.id,
                Some("ctx-reconcile-complete"),
                json!({"prompt":"go"}),
            )
            .await
            .expect("create run");
        let step = teams
            .submit_step(
                &run.id,
                "plan",
                "planner",
                Vec::new(),
                Some(json!({"goal":"plan"})),
            )
            .await
            .expect("submit step");
        let _ = teams
            .start_step(&step.id, Some("session-complete"))
            .await
            .expect("start step");

        sqlx::query(
            r#"
            INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, ?3, 1, 2)
            "#,
        )
        .bind("session-complete")
        .bind("planner")
        .bind("completed")
        .execute(&db)
        .await
        .expect("insert completed agent session");

        let starter = Arc::new(FakeAgentStarter::default());
        let worker = TeamOrchestratorWorker::with_agent_starter(teams.clone(), starter.clone());
        let summary = worker.dispatch_once(10).await.expect("dispatch once");

        assert_eq!(summary.dispatched, 0);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.reconciled_completed, 1);
        assert_eq!(summary.reconciled_failed, 0);
        assert!(starter.calls().is_empty());

        let completed_step = teams.get_step(&step.id).await.expect("get completed step");
        assert_eq!(completed_step.status, TeamStepStatus::Completed);
        let completed_run = teams.get_run(&run.id).await.expect("get completed run");
        assert_eq!(completed_run.status, crate::team::TeamRunStatus::Completed);
    }

    #[tokio::test]
    async fn dispatch_once_reconciles_working_step_from_failed_session() {
        let db = setup_test_db().await;
        let teams = Arc::new(TeamManager::new(db.clone()));
        let team = teams
            .create_team(TeamDefinitionConfig {
                name: "orchestrator-reconcile-fail-team".to_string(),
                description: Some("team for orchestrator reconcile failure".to_string()),
                spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
            })
            .await
            .expect("create team");
        let run = teams
            .create_run(&team.id, Some("ctx-reconcile-fail"), json!({"prompt":"go"}))
            .await
            .expect("create run");
        let step = teams
            .submit_step(
                &run.id,
                "plan",
                "planner",
                Vec::new(),
                Some(json!({"goal":"plan"})),
            )
            .await
            .expect("submit step");
        let _ = teams
            .start_step(&step.id, Some("session-failed"))
            .await
            .expect("start step");

        sqlx::query(
            r#"
            INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, ?3, 1, 2)
            "#,
        )
        .bind("session-failed")
        .bind("planner")
        .bind("failed")
        .execute(&db)
        .await
        .expect("insert failed agent session");

        let starter = Arc::new(FakeAgentStarter::default());
        let worker = TeamOrchestratorWorker::with_agent_starter(teams.clone(), starter.clone());
        let summary = worker.dispatch_once(10).await.expect("dispatch once");

        assert_eq!(summary.dispatched, 0);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.reconciled_completed, 0);
        assert_eq!(summary.reconciled_failed, 1);
        assert!(starter.calls().is_empty());

        let failed_step = teams.get_step(&step.id).await.expect("get failed step");
        assert_eq!(failed_step.status, TeamStepStatus::Failed);
        assert!(
            failed_step
                .error_text
                .as_deref()
                .is_some_and(|text| text.contains("session-failed"))
        );
        let failed_run = teams.get_run(&run.id).await.expect("get failed run");
        assert_eq!(failed_run.status, crate::team::TeamRunStatus::Failed);
    }

    #[tokio::test]
    async fn dispatch_once_handles_input_required_resume_with_idempotent_retries() {
        let db = setup_test_db().await;
        let teams = Arc::new(TeamManager::new(db.clone()));
        let team = teams
            .create_team(TeamDefinitionConfig {
                name: "orchestrator-input-required-resume-team".to_string(),
                description: Some("team for input_required/resume reconciliation".to_string()),
                spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
            })
            .await
            .expect("create team");
        let run = teams
            .create_run(&team.id, Some("ctx-input-required"), json!({"prompt":"go"}))
            .await
            .expect("create run");
        let step = teams
            .submit_step(
                &run.id,
                "plan",
                "planner",
                Vec::new(),
                Some(json!({"goal":"plan"})),
            )
            .await
            .expect("submit step");
        let _ = teams
            .start_step(&step.id, Some("session-input-resume"))
            .await
            .expect("start step");
        let _ = teams
            .set_step_input_required(
                &step.id,
                Some("need approval"),
                Some(json!({"question":"approve?"})),
            )
            .await
            .expect("set input_required");
        let input_retry = teams
            .set_step_input_required(
                &step.id,
                Some("retry should be idempotent"),
                Some(json!({"question":"approve?"})),
            )
            .await
            .expect("retry input_required");
        assert_eq!(input_retry.status, TeamStepStatus::InputRequired);
        assert_eq!(input_retry.error_text.as_deref(), Some("need approval"));

        sqlx::query(
            r#"
            INSERT INTO agent_sessions (id, agent_id, status, started_at)
            VALUES (?1, ?2, ?3, 1)
            "#,
        )
        .bind("session-input-resume")
        .bind("planner")
        .bind("running")
        .execute(&db)
        .await
        .expect("insert running session");

        let starter = Arc::new(FakeAgentStarter::default());
        let worker = TeamOrchestratorWorker::with_agent_starter(teams.clone(), starter.clone());

        let summary_before_resume = worker
            .dispatch_once(10)
            .await
            .expect("dispatch before resume");
        assert_eq!(summary_before_resume.reconciled_completed, 0);
        assert_eq!(summary_before_resume.reconciled_failed, 0);
        assert!(starter.calls().is_empty());
        let step_before_resume = teams
            .get_step(&step.id)
            .await
            .expect("get input_required step");
        assert_eq!(step_before_resume.status, TeamStepStatus::InputRequired);

        let resumed = teams
            .resume_step(&step.id, Some(json!({"answer":"approved"})))
            .await
            .expect("resume step");
        assert_eq!(resumed.status, TeamStepStatus::Working);
        assert_eq!(resumed.input, Some(json!({"answer":"approved"})));
        let resumed_retry = teams
            .resume_step(&step.id, Some(json!({"answer":"approved-again"})))
            .await
            .expect("retry resume step");
        assert_eq!(resumed_retry.status, TeamStepStatus::Working);
        assert_eq!(resumed_retry.input, Some(json!({"answer":"approved"})));

        sqlx::query(
            r#"
            UPDATE agent_sessions
            SET status = 'completed', ended_at = 2
            WHERE id = ?1
            "#,
        )
        .bind("session-input-resume")
        .execute(&db)
        .await
        .expect("mark session completed");

        let summary_after_resume = worker
            .dispatch_once(10)
            .await
            .expect("dispatch after resume");
        assert_eq!(summary_after_resume.reconciled_completed, 1);
        assert_eq!(summary_after_resume.reconciled_failed, 0);
        assert!(starter.calls().is_empty());

        let completed_step = teams.get_step(&step.id).await.expect("get completed step");
        assert_eq!(completed_step.status, TeamStepStatus::Completed);
        let completed_run = teams.get_run(&run.id).await.expect("get completed run");
        assert_eq!(completed_run.status, crate::team::TeamRunStatus::Completed);
    }

    #[tokio::test]
    async fn dispatch_once_reconciles_input_required_step_from_failed_session() {
        let db = setup_test_db().await;
        let teams = Arc::new(TeamManager::new(db.clone()));
        let team = teams
            .create_team(TeamDefinitionConfig {
                name: "orchestrator-input-required-fail-team".to_string(),
                description: Some("team for input_required failed session reconcile".to_string()),
                spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
            })
            .await
            .expect("create team");
        let run = teams
            .create_run(
                &team.id,
                Some("ctx-input-required-fail"),
                json!({"prompt":"go"}),
            )
            .await
            .expect("create run");
        let step = teams
            .submit_step(
                &run.id,
                "plan",
                "planner",
                Vec::new(),
                Some(json!({"goal":"plan"})),
            )
            .await
            .expect("submit step");
        let _ = teams
            .start_step(&step.id, Some("session-input-fail"))
            .await
            .expect("start step");
        let _ = teams
            .set_step_input_required(
                &step.id,
                Some("waiting for review"),
                Some(json!({"question":"approve?"})),
            )
            .await
            .expect("set input_required");

        sqlx::query(
            r#"
            INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, ?3, 1, 2)
            "#,
        )
        .bind("session-input-fail")
        .bind("planner")
        .bind("exited")
        .execute(&db)
        .await
        .expect("insert exited session");

        let starter = Arc::new(FakeAgentStarter::default());
        let worker = TeamOrchestratorWorker::with_agent_starter(teams.clone(), starter.clone());
        let summary = worker.dispatch_once(10).await.expect("dispatch once");

        assert_eq!(summary.dispatched, 0);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.reconciled_completed, 0);
        assert_eq!(summary.reconciled_failed, 1);
        assert!(starter.calls().is_empty());

        let failed_step = teams.get_step(&step.id).await.expect("get failed step");
        assert_eq!(failed_step.status, TeamStepStatus::Failed);
        assert!(
            failed_step
                .error_text
                .as_deref()
                .is_some_and(|text| text.contains("session-input-fail"))
        );
        let failed_run = teams.get_run(&run.id).await.expect("get failed run");
        assert_eq!(failed_run.status, crate::team::TeamRunStatus::Failed);
    }

    #[test]
    fn orchestrator_settings_have_expected_defaults() {
        let settings = TeamOrchestratorWorkerSettings::default();
        assert_eq!(settings.poll_interval_secs, 2);
        assert_eq!(settings.max_dispatch_per_tick, 32);
        assert_eq!(settings.heartbeat_interval_ticks, 150);
    }
}
