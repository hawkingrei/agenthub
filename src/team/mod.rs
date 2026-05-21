mod mailbox_hint;
mod manager;
mod permission_review;
mod reconcile_prompt;
mod role_skills;
mod runtime;

use std::collections::HashSet;

use crate::acp::{AcpActorSkillContext, DEFAULT_ACTOR_CHANNEL};
pub use agenthub_team_actor::{
    ActorMessageRecord as TeamActorMessageRecord, ActorMessageStatus as TeamActorMessageStatus,
    ActorMessageTransport as TeamActorMessageTransport,
};
pub use agenthub_team_domain::{
    TEAM_RUN_CONTINUITY_MODE_VALUES, TEAM_RUN_STATUS_VALUES, TEAM_STEP_STATUS_VALUES,
    TEAM_TASK_NOTE_KIND_VALUES, TEAM_TASK_PRIORITY_VALUES, TEAM_TASK_STATUS_VALUES,
    TeamChannelRecord, TeamConversationMessageRecord, TeamConversationRecord, TeamDefinitionConfig,
    TeamDefinitionRecord, TeamMemberContinuityStateRecord, TeamRunEventRecord, TeamRunRecord,
    TeamRunResumeError, TeamRunStatus, TeamStepRecord, TeamStepStatus, TeamTaskDetailRecord,
    TeamTaskExecutionPlan, TeamTaskNoteKind, TeamTaskNoteRecord, TeamTaskPriority, TeamTaskRecord,
    TeamTaskStatus, TeamTaskStepExecutionMode, TeamTaskStepExecutionSpec, TeamThreadOpenRecord,
    TeamThreadReplyRecord,
};
#[cfg(test)]
pub(crate) use mailbox_hint::build_actor_mailbox_immediate_hint_prompt;
pub(crate) use mailbox_hint::{
    ActorMailboxImmediateHintReason, TeamMailboxUnreadHintWorker,
    TeamMailboxUnreadHintWorkerSettings, dispatch_actor_mailbox_immediate_hint,
    plan_actor_mailbox_immediate_hint,
};
pub(crate) use manager::TEAM_TASK_DETAIL_MESSAGE_LIMIT_MAX;
#[allow(unused_imports)]
pub use manager::{
    SendActorMessageInput, TeamContextLookupError, TeamContextRecord, TeamConversationStreamEvent,
    TeamManager, TeamMemoryFlushRequest, TeamMessageArchiveMigrationReport,
    TeamRemoteRelayWorkerSettings, TeamReplyObligationRecord, TeamRunContextFingerprint,
    TeamRunContextStreamEvent, TeamRuntimeRecord, TeamRuntimeStatus, TeamTaskAssignmentUpdate,
    TeamTaskContextPatch, TeamTaskCreateInput, TeamTaskListQuery, TeamTaskNoteCreateInput,
    TeamTaskUpdateWithNoteInput,
};
pub(crate) use permission_review::resolve_team_permission_review_target;
pub use permission_review::{
    TeamPermissionReviewDispatcher, TeamPermissionReviewDispatcherSettings,
};
pub(crate) use reconcile_prompt::maybe_nudge_reconcile_step_prompt;
pub use role_skills::effective_team_member_skills;
pub use runtime::{
    TeamRuntimeControlRecord, TeamRuntimeStartError, ensure_team_runtime_started,
    force_team_member_new_session, stop_team_runtime,
};
use serde_json::Value;

pub(crate) fn parse_task_execution_plan(
    context: &Value,
) -> anyhow::Result<Option<TeamTaskExecutionPlan>> {
    let Some(context_obj) = context.as_object() else {
        return Ok(None);
    };
    let Some(raw_plan) = context_obj.get("execution_plan") else {
        return Ok(None);
    };
    Ok(Some(serde_json::from_value(raw_plan.clone())?))
}

pub(crate) fn validate_task_execution_plan(
    team_spec: &Value,
    context: &Value,
) -> anyhow::Result<Option<TeamTaskExecutionPlan>> {
    let Some(plan) = parse_task_execution_plan(context)? else {
        return Ok(None);
    };
    if plan.steps.is_empty() {
        anyhow::bail!("task context execution_plan.steps must not be empty");
    }
    validate_task_execution_steps(team_spec, &plan.steps, "task context execution_plan.steps")?;
    Ok(Some(plan))
}

pub(crate) fn validate_task_execution_steps(
    team_spec: &Value,
    steps: &[agenthub_team_domain::TeamTaskExecutionStepSpec],
    scope: &str,
) -> anyhow::Result<()> {
    let member_ids = collect_team_member_ids(team_spec)
        .into_iter()
        .collect::<HashSet<_>>();
    let mut seen_step_keys = HashSet::with_capacity(steps.len());
    for step in steps {
        let step_key = step.step_key.trim();
        if step_key.is_empty() {
            anyhow::bail!("{scope}[].step_key is required");
        }
        if !seen_step_keys.insert(step_key.to_string()) {
            anyhow::bail!("{scope}[].step_key must be unique");
        }

        let member_id = step.member_id.trim();
        if member_id.is_empty() {
            anyhow::bail!("{scope}[].member_id is required");
        }
        if !member_ids.contains(member_id) {
            anyhow::bail!("{scope}[].member_id must reference spec.members[].member_id");
        }

        let mut seen_depends_on = HashSet::with_capacity(step.depends_on.len());
        for dependency in &step.depends_on {
            let dependency = dependency.trim();
            if dependency.is_empty() {
                anyhow::bail!("{scope}[].depends_on entries must be non-empty");
            }
            if dependency == step_key {
                anyhow::bail!("{scope}[].depends_on must not self-reference");
            }
            if !seen_depends_on.insert(dependency.to_string()) {
                anyhow::bail!("{scope}[].depends_on must not contain duplicates");
            }
        }

        if step.execution.mode == TeamTaskStepExecutionMode::ReconcileLoop {
            let goal = step
                .goal
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty());
            if goal.is_none() {
                anyhow::bail!("{scope} reconcile_loop steps require a non-empty goal");
            }
            if step.acceptance.iter().all(|item| item.trim().is_empty()) {
                anyhow::bail!("{scope} reconcile_loop steps require at least one acceptance item");
            }
            if let Some(max_rounds) = step.execution.max_rounds
                && !(1..=32).contains(&max_rounds)
            {
                anyhow::bail!("{scope}[].execution.max_rounds must be between 1 and 32");
            }
        }
    }

    for step in steps {
        for dependency in &step.depends_on {
            if !seen_step_keys.contains(dependency.trim()) {
                anyhow::bail!("{scope}[].depends_on must reference existing step_key");
            }
        }
    }
    ensure_acyclic_task_execution_steps(steps, scope)?;
    Ok(())
}

fn ensure_acyclic_task_execution_steps(
    steps: &[agenthub_team_domain::TeamTaskExecutionStepSpec],
    scope: &str,
) -> anyhow::Result<()> {
    let mut indegree = std::collections::HashMap::with_capacity(steps.len());
    let mut dependents = std::collections::HashMap::with_capacity(steps.len());
    for step in steps {
        let step_key = step.step_key.trim().to_string();
        indegree.insert(step_key.clone(), step.depends_on.len());
        for dependency in &step.depends_on {
            dependents
                .entry(dependency.trim().to_string())
                .or_insert_with(Vec::new)
                .push(step_key.clone());
        }
    }

    let mut ready = std::collections::VecDeque::new();
    for (step_key, degree) in &indegree {
        if *degree == 0 {
            ready.push_back(step_key.clone());
        }
    }

    let mut visited = 0usize;
    while let Some(step_key) = ready.pop_front() {
        visited += 1;
        if let Some(children) = dependents.get(&step_key) {
            for child in children {
                let degree = indegree
                    .get_mut(child)
                    .expect("child indegree should exist for validated task execution plan");
                *degree -= 1;
                if *degree == 0 {
                    ready.push_back(child.clone());
                }
            }
        }
    }

    if visited != steps.len() {
        anyhow::bail!("{scope} must be acyclic");
    }
    Ok(())
}

pub(crate) fn collect_team_member_ids(spec: &Value) -> Vec<String> {
    let Some(members) = spec.get("members").and_then(Value::as_array) else {
        return Vec::new();
    };

    let mut member_ids = Vec::with_capacity(members.len());
    let mut seen = HashSet::with_capacity(members.len());
    for member in members {
        let Some(member_id) = member
            .get("member_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if seen.insert(member_id) {
            member_ids.push(member_id.to_string());
        }
    }
    member_ids
}

pub(crate) fn team_member_role_from_spec(spec: &Value, member_id: &str) -> Option<String> {
    let member_id = member_id.trim();
    let members = spec.get("members")?.as_array()?;
    members
        .iter()
        .find(|member| {
            member
                .get("member_id")
                .and_then(Value::as_str)
                .map(str::trim)
                == Some(member_id)
        })
        .and_then(|member| member.get("role"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|role| !role.is_empty())
        .map(str::to_string)
}

pub(crate) fn build_team_member_actor_context_for_role(
    team_id: &str,
    run_id: Option<&str>,
    member_id: &str,
    member_role: &str,
) -> AcpActorSkillContext {
    AcpActorSkillContext {
        team_id: Some(team_id.to_string()),
        current_run_id: run_id.map(str::to_string),
        actor_id: member_id.to_string(),
        default_channel: DEFAULT_ACTOR_CHANNEL.to_string(),
        member_role: Some(member_role.to_string()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    }
}

pub(crate) fn normalize_optional_idempotency_key_input(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
