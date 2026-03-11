use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::acp::{AcpActorSkillContext, DEFAULT_ACTOR_CHANNEL, default_actor_cli_path};
use crate::agent::AgentManager;
use crate::team::{TeamDefinitionRecord, TeamRuntimeStatus};

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
    skills: Vec<String>,
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
        out.push(TeamRuntimeMemberSpec {
            member_id: member_id.to_string(),
            role: role.to_string(),
            skills,
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

pub async fn ensure_team_runtime_started(
    agents: &AgentManager,
    team: &TeamDefinitionRecord,
) -> anyhow::Result<TeamRuntimeControlRecord> {
    let member_specs = parse_runtime_member_specs(&team.spec)?;
    let actor_cli_path = default_actor_cli_path()?;
    let mut started_member_ids = Vec::new();
    let mut members = Vec::with_capacity(member_specs.len());

    for member in &member_specs {
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
