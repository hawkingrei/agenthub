use anyhow::Context;
use sqlx::Error as SqlxError;

use crate::agent::{AgentManager, AgentRecord, WorktreeMode, normalize_target_node_id};

use super::spec::{TeamRuntimeMemberRuntimeHint, TeamRuntimeMemberSpec, trimmed_opt};
use super::types::TeamRuntimeStartError;

#[derive(Debug, Clone)]
pub(super) struct WorkerRuntimeRepairConfig {
    pub workdir: String,
    pub worktree_repo: String,
    pub worktree_ref: Option<String>,
}

fn worker_runtime_is_valid(agent: &AgentRecord) -> bool {
    matches!(agent.worktree_mode, WorktreeMode::UseExisting)
        || (matches!(agent.worktree_mode, WorktreeMode::CreateWorktree)
            && trimmed_opt(agent.worktree_repo.as_deref()).is_some())
}

fn build_worker_runtime_hint_config(
    agent: &AgentRecord,
    hint: &TeamRuntimeMemberRuntimeHint,
) -> Option<WorkerRuntimeRepairConfig> {
    let worktree_repo = trimmed_opt(hint.worktree_repo.as_deref())?;
    let workdir = trimmed_opt(hint.workdir.as_deref()).unwrap_or_else(|| agent.workdir.clone());
    let worktree_ref = trimmed_opt(hint.worktree_ref.as_deref())
        .or_else(|| trimmed_opt(agent.worktree_ref.as_deref()))
        .or_else(|| Some("HEAD".to_string()));
    Some(WorkerRuntimeRepairConfig {
        workdir,
        worktree_repo,
        worktree_ref,
    })
}

fn build_worker_runtime_agent_config(agent: &AgentRecord) -> Option<WorkerRuntimeRepairConfig> {
    let worktree_repo = trimmed_opt(agent.worktree_repo.as_deref())?;
    let worktree_ref =
        trimmed_opt(agent.worktree_ref.as_deref()).or_else(|| Some("HEAD".to_string()));
    Some(WorkerRuntimeRepairConfig {
        workdir: agent.workdir.clone(),
        worktree_repo,
        worktree_ref,
    })
}

fn resolve_worker_runtime_repair(
    member: &TeamRuntimeMemberSpec,
    agent: &AgentRecord,
) -> Option<WorkerRuntimeRepairConfig> {
    if let Some(hint) = member.runtime.as_ref()
        && let Some(config) = build_worker_runtime_hint_config(agent, hint)
    {
        return Some(config);
    }
    build_worker_runtime_agent_config(agent)
}

pub(super) async fn reconcile_team_member_runtime(
    agents: &AgentManager,
    member: &TeamRuntimeMemberSpec,
) -> anyhow::Result<()> {
    let agent = agents
        .get_agent(member.member_id.as_str())
        .await
        .map_err(|err| map_member_agent_lookup_error(member.member_id.as_str(), err))?;
    if let Some(runtime) = member.runtime.as_ref()
        && runtime_target_node_hint_is_present(runtime.target_node_id.as_deref())
    {
        let expected_target_node_id = normalize_target_node_id(runtime.target_node_id.as_deref());
        let actual_target_node_id = normalize_target_node_id(agent.target_node_id.as_deref());
        if actual_target_node_id != expected_target_node_id {
            return Err(TeamRuntimeStartError::InvalidConfig(format!(
                "team member '{}' expects target_node_id '{}' but agent '{}' is bound to '{}'",
                member.member_id,
                expected_target_node_id.as_deref().unwrap_or("main"),
                agent.id,
                actual_target_node_id.as_deref().unwrap_or("main")
            ))
            .into());
        }
    }
    if member.role != "worker" || worker_runtime_is_valid(&agent) {
        return Ok(());
    }

    let Some(repair) = resolve_worker_runtime_repair(member, &agent) else {
        return Err(TeamRuntimeStartError::InvalidConfig(format!(
            "team member runtime config for worker '{}' is missing worktree_repo; reconfigure the team member agent or recreate the team",
            member.member_id
        ))
        .into());
    };

    agents
        .update_team_member_runtime_config(
            agent.id.as_str(),
            repair.workdir.as_str(),
            WorktreeMode::CreateWorktree,
            Some(repair.worktree_repo.as_str()),
            repair.worktree_ref.as_deref(),
        )
        .await
        .with_context(|| format!("repair worker runtime config for '{}'", member.member_id))?;
    Ok(())
}

pub(super) fn runtime_target_node_hint_is_present(raw: Option<&str>) -> bool {
    trimmed_opt(raw).is_some()
}

fn is_row_not_found_error(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<SqlxError>(),
        Some(SqlxError::RowNotFound)
    )
}

pub(super) fn map_member_agent_lookup_error(member_id: &str, err: anyhow::Error) -> anyhow::Error {
    if is_row_not_found_error(&err) {
        return TeamRuntimeStartError::MissingMemberAgent(format!(
            "team member agent '{}' not found",
            member_id
        ))
        .into();
    }

    err.context(format!("load team member agent '{}'", member_id))
}
