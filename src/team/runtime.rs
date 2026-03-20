use std::collections::HashSet;
use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::acp::{AcpActorSkillContext, DEFAULT_ACTOR_CHANNEL, default_actor_cli_path};
use crate::agent::{AgentManager, AgentRecord, WorktreeMode};
use crate::path_utils::{expand_tilde, is_path_allowed, normalize_path};
use crate::team::{TeamDefinitionRecord, TeamRuntimeStatus};

#[derive(Debug, thiserror::Error)]
pub enum TeamRuntimeStartError {
    #[error("{0}")]
    InvalidConfig(String),
    #[error("{0}")]
    MissingMemberAgent(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamRuntimeMemberStatusRecord {
    pub member_id: String,
    pub session_id: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamRuntimeControlRecord {
    pub team_id: String,
    pub status: TeamRuntimeStatus,
    pub members: Vec<TeamRuntimeMemberStatusRecord>,
}

#[derive(Debug, Clone)]
struct TeamRuntimeMemberSpec {
    member_id: String,
    role: String,
    description: Option<String>,
    prompt: Option<String>,
    skills: Vec<String>,
    runtime: Option<TeamRuntimeMemberRuntimeHint>,
}

#[derive(Debug, Clone, Deserialize)]
struct TeamRuntimeMemberRuntimeHint {
    #[allow(dead_code)]
    name: Option<String>,
    workdir: Option<String>,
    worktree_repo: Option<String>,
    worktree_ref: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    agent_loop_enabled: Option<bool>,
    #[serde(default)]
    #[allow(dead_code)]
    agent_loop_idle_seconds: Option<i64>,
    #[serde(default)]
    #[allow(dead_code)]
    agent_loop_prompt: Option<String>,
}

#[derive(Debug, Clone)]
struct WorkerRuntimeRepairConfig {
    workdir: String,
    worktree_repo: String,
    worktree_ref: Option<String>,
}

fn parse_runtime_member_specs(spec: &Value) -> anyhow::Result<Vec<TeamRuntimeMemberSpec>> {
    let members = spec
        .get("members")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("spec.members must be an array"))?;
    let mut out = Vec::with_capacity(members.len());
    for member in members {
        let member_obj = member
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("spec.members entries must be objects"))?;
        let member_id = member_obj
            .get("member_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("spec.members[].member_id is required"))?;
        let role = member_obj
            .get("role")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("worker");
        let skills = member_obj
            .get("skills")
            .and_then(Value::as_array)
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let description = member_obj
            .get("description")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let prompt = member_obj
            .get("prompt")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let runtime = member_obj
            .get("runtime")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .context("parse spec.members[].runtime")?;
        out.push(TeamRuntimeMemberSpec {
            member_id: member_id.to_string(),
            role: role.to_string(),
            description,
            prompt,
            skills,
            runtime,
        });
    }
    Ok(out)
}

fn build_team_member_actor_context(
    team_id: &str,
    member: &TeamRuntimeMemberSpec,
    actor_cli_path: &str,
) -> anyhow::Result<AcpActorSkillContext> {
    Ok(AcpActorSkillContext {
        team_id: Some(team_id.to_string()),
        current_run_id: None,
        actor_id: member.member_id.clone(),
        default_channel: DEFAULT_ACTOR_CHANNEL.to_string(),
        actor_cli_path: actor_cli_path.to_string(),
        member_role: Some(member.role.clone()),
        member_skills: member.skills.clone(),
        contract_version: None,
        continuity: None,
    })
}

fn team_member_actor_context_matches(
    current: Option<&AcpActorSkillContext>,
    expected: &AcpActorSkillContext,
) -> bool {
    let Some(current) = current else {
        return false;
    };
    current.team_id == expected.team_id
        && current.current_run_id == expected.current_run_id
        && current.actor_id == expected.actor_id
        && current.default_channel == expected.default_channel
        && current.member_role == expected.member_role
        && current.member_skills == expected.member_skills
}

fn trimmed_opt(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
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

fn adjust_worker_runtime_workdir_for_safe_paths(
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

async fn reconcile_team_member_runtime(
    agents: &AgentManager,
    member: &TeamRuntimeMemberSpec,
) -> anyhow::Result<()> {
    let agent = agents
        .get_agent(member.member_id.as_str())
        .await
        .map_err(|_| {
            TeamRuntimeStartError::MissingMemberAgent(format!(
                "team member agent '{}' not found",
                member.member_id
            ))
        })?;
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

pub async fn ensure_team_runtime_started(
    agents: &AgentManager,
    team: &TeamDefinitionRecord,
) -> anyhow::Result<TeamRuntimeControlRecord> {
    let member_specs = parse_runtime_member_specs(&team.spec)?;
    let actor_cli_path = default_actor_cli_path()?;
    let mut started_member_ids = Vec::new();
    let mut members = Vec::with_capacity(member_specs.len());

    for member in &member_specs {
        reconcile_team_member_runtime(agents, member)
            .await
            .with_context(|| format!("prepare runtime for member '{}'", member.member_id))?;
        let actor_context =
            build_team_member_actor_context(team.id.as_str(), member, actor_cli_path.as_str())
                .with_context(|| {
                    format!("build actor context for member '{}'", member.member_id)
                })?;
        let mut action = "started";
        if let Some(session_id) = agents
            .running_session_id_for_agent(member.member_id.as_str())
            .await
        {
            let running_context = agents
                .running_actor_context_for_agent(member.member_id.as_str())
                .await;
            if team_member_actor_context_matches(running_context.as_ref(), &actor_context) {
                members.push(TeamRuntimeMemberStatusRecord {
                    member_id: member.member_id.clone(),
                    session_id,
                    action: "reused".to_string(),
                });
                continue;
            }
            action = "restarted";
            agents
                .stop_agent(member.member_id.as_str())
                .await
                .with_context(|| format!("stop stale runtime for member '{}'", member.member_id))?;
        }

        match agents
            .start_agent_with_actor_context(member.member_id.as_str(), Some(actor_context))
            .await
        {
            Ok(session_id) => {
                started_member_ids.push(member.member_id.clone());
                members.push(TeamRuntimeMemberStatusRecord {
                    member_id: member.member_id.clone(),
                    session_id,
                    action: action.to_string(),
                });
            }
            Err(err) => {
                for started_member_id in &started_member_ids {
                    let _ = agents.stop_agent(started_member_id.as_str()).await;
                }
                return Err(anyhow::anyhow!(
                    "failed to start team member runtime '{}' for team '{}': {}",
                    member.member_id,
                    team.id,
                    err
                ));
            }
        }
    }

    Ok(TeamRuntimeControlRecord {
        team_id: team.id.clone(),
        status: TeamRuntimeStatus::Running,
        members,
    })
}

pub async fn stop_team_runtime(
    agents: &AgentManager,
    team: &TeamDefinitionRecord,
) -> anyhow::Result<TeamRuntimeControlRecord> {
    let member_specs = parse_runtime_member_specs(&team.spec)?;
    let mut members = Vec::new();
    for member in &member_specs {
        let Some(session_id) = agents
            .running_session_id_for_agent(member.member_id.as_str())
            .await
        else {
            continue;
        };
        agents
            .stop_agent(member.member_id.as_str())
            .await
            .with_context(|| format!("stop team member runtime '{}'", member.member_id))?;
        members.push(TeamRuntimeMemberStatusRecord {
            member_id: member.member_id.clone(),
            session_id,
            action: "stopped".to_string(),
        });
    }
    Ok(TeamRuntimeControlRecord {
        team_id: team.id.clone(),
        status: TeamRuntimeStatus::Stopped,
        members,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TeamRuntimeStartError, WorkerRuntimeRepairConfig,
        adjust_worker_runtime_workdir_for_safe_paths,
    };
    use crate::path_utils::expand_tilde;

    #[test]
    fn expand_tilde_uses_path_join_for_home_relative_paths() {
        let home = std::env::var("HOME").expect("HOME");
        assert_eq!(
            expand_tilde("~/worktrees"),
            std::path::Path::new(&home)
                .join("worktrees")
                .to_string_lossy()
                .to_string()
        );
    }

    #[test]
    fn worker_runtime_adjust_rejects_workdir_outside_safe_paths() {
        let err = adjust_worker_runtime_workdir_for_safe_paths(
            WorkerRuntimeRepairConfig {
                workdir: "/tmp/agenthub-worker".to_string(),
                worktree_repo: "/repo".to_string(),
                worktree_ref: Some("HEAD".to_string()),
            },
            &[String::from("/safe/root")],
        )
        .expect_err("workdir outside safe paths should fail");
        let typed = err
            .downcast_ref::<TeamRuntimeStartError>()
            .expect("typed runtime error");
        assert!(matches!(typed, TeamRuntimeStartError::InvalidConfig(_)));
    }
}
