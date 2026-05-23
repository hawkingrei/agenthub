use std::collections::HashSet;
use std::path::Path;

use anyhow::Context;
use sqlx::Error as SqlxError;

use crate::agent::{AgentManager, AgentRecord, WorktreeMode, normalize_target_node_id};
use crate::path_utils::{expand_tilde, is_path_allowed, normalize_path};

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

fn is_git_repo(path: &Path) -> bool {
    path.join(".git").exists()
}

fn collect_repo_candidates(safe_paths: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();
    for safe_path in safe_paths {
        let root = expand_tilde(safe_path);
        let root_path = Path::new(&root);
        if is_git_repo(root_path) && seen.insert(root.clone()) {
            candidates.push(root.clone());
        }
        let Ok(entries) = std::fs::read_dir(root_path) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() || !is_git_repo(&path) {
                continue;
            }
            let candidate = path.to_string_lossy().to_string();
            if seen.insert(candidate.clone()) {
                candidates.push(candidate);
            }
        }
    }
    candidates
}

fn safe_paths_allow(safe_paths: &[String], target: &str) -> bool {
    let expanded_target = expand_tilde(target);
    safe_paths.iter().any(|allowed| {
        let expanded_allowed = expand_tilde(allowed);
        is_path_allowed(&expanded_target, &expanded_allowed)
    })
}

pub(super) fn adjust_worker_runtime_workdir_for_safe_paths(
    mut config: WorkerRuntimeRepairConfig,
    safe_paths: &[String],
) -> anyhow::Result<WorkerRuntimeRepairConfig> {
    let derived_root = Path::new(&config.workdir)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(|parent| parent.to_string_lossy().to_string())
        .unwrap_or_else(|| config.workdir.clone());
    if safe_paths_allow(safe_paths, &derived_root) {
        return Ok(config);
    }

    if safe_paths_allow(safe_paths, &config.workdir) {
        let normalized_repo = normalize_path(&expand_tilde(&config.worktree_repo));
        let normalized_workdir = normalize_path(&expand_tilde(&config.workdir));
        if is_path_allowed(&normalized_workdir, &normalized_repo) {
            return Err(TeamRuntimeStartError::InvalidConfig(format!(
                "legacy worker runtime workdir '{}' is inside repo '{}' and cannot derive a safe worktree root",
                config.workdir, config.worktree_repo
            ))
            .into());
        }
        config.workdir = Path::new(&config.workdir)
            .join(".agenthub-worker-root")
            .to_string_lossy()
            .to_string();
        return Ok(config);
    }

    Err(TeamRuntimeStartError::InvalidConfig(format!(
        "legacy worker runtime workdir '{}' is outside safe paths and cannot derive a safe worktree root",
        config.workdir
    ))
    .into())
}

fn infer_repo_from_member_text(
    candidates: &[String],
    member: &TeamRuntimeMemberSpec,
    agent: &AgentRecord,
) -> Option<String> {
    let mut haystack_parts = Vec::new();
    if let Some(prompt) = member.prompt.as_deref() {
        haystack_parts.push(prompt.to_ascii_lowercase());
    }
    if let Some(description) = member.description.as_deref() {
        haystack_parts.push(description.to_ascii_lowercase());
    }
    if let Some(runtime) = member.runtime.as_ref()
        && let Some(name) = runtime.name.as_deref()
    {
        haystack_parts.push(name.to_ascii_lowercase());
    }
    haystack_parts.push(member.member_id.to_ascii_lowercase());
    haystack_parts.push(agent.name.to_ascii_lowercase());
    let haystack = haystack_parts.join("\n");
    let matches = candidates
        .iter()
        .filter_map(|candidate| {
            let basename = Path::new(candidate)
                .file_name()
                .and_then(|value| value.to_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())?
                .to_ascii_lowercase();
            haystack.contains(&basename).then(|| candidate.clone())
        })
        .collect::<Vec<_>>();
    (matches.len() == 1).then(|| matches[0].clone())
}

async fn resolve_worker_runtime_repair(
    agents: &AgentManager,
    member: &TeamRuntimeMemberSpec,
    agent: &AgentRecord,
) -> anyhow::Result<Option<WorkerRuntimeRepairConfig>> {
    let safe_paths = agents.list_safe_paths().await?;
    if let Some(hint) = member.runtime.as_ref()
        && let Some(config) = build_worker_runtime_hint_config(agent, hint)
    {
        let config = adjust_worker_runtime_workdir_for_safe_paths(config, &safe_paths)?;
        return Ok(Some(config));
    }
    if let Some(config) = build_worker_runtime_agent_config(agent) {
        let config = adjust_worker_runtime_workdir_for_safe_paths(config, &safe_paths)?;
        return Ok(Some(config));
    }
    let candidates = collect_repo_candidates(&safe_paths);
    let inferred_repo = infer_repo_from_member_text(&candidates, member, agent);
    inferred_repo
        .map(|worktree_repo| WorkerRuntimeRepairConfig {
            workdir: agent.workdir.clone(),
            worktree_repo,
            worktree_ref: Some("HEAD".to_string()),
        })
        .map(|config| adjust_worker_runtime_workdir_for_safe_paths(config, &safe_paths))
        .transpose()
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

    let Some(repair) = resolve_worker_runtime_repair(agents, member, &agent).await? else {
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
