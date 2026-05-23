use anyhow::Context;

use crate::agent::AgentManager;
use crate::team::TeamDefinitionRecord;

use super::repair::{map_member_agent_lookup_error, reconcile_team_member_runtime};
use super::spec::{
    build_team_member_actor_context, parse_runtime_member_specs, team_member_actor_context_matches,
};
use super::types::{
    TeamRuntimeControlRecord, TeamRuntimeMemberStatusRecord, TeamRuntimeStartError,
};
use crate::team::TeamRuntimeStatus;

pub async fn ensure_team_runtime_started(
    agents: &AgentManager,
    team: &TeamDefinitionRecord,
) -> anyhow::Result<TeamRuntimeControlRecord> {
    let member_specs = parse_runtime_member_specs(&team.spec)?;
    let mut started_member_ids = Vec::new();
    let mut members = Vec::with_capacity(member_specs.len());

    for member in &member_specs {
        reconcile_team_member_runtime(agents, member)
            .await
            .with_context(|| format!("prepare runtime for member '{}'", member.member_id))?;
        let actor_context = build_team_member_actor_context(team.id.as_str(), member)
            .with_context(|| format!("build actor context for member '{}'", member.member_id))?;
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
                tracing::error!(
                    team_id = %team.id,
                    member_id = %member.member_id,
                    error = %err,
                    "failed to start team member runtime"
                );
                for started_member_id in &started_member_ids {
                    let _ = agents.stop_agent(started_member_id.as_str()).await;
                }
                return Err(TeamRuntimeStartError::MemberRuntimeStart(format!(
                    "failed to start team member runtime '{}' for team '{}'",
                    member.member_id, team.id
                ))
                .into());
            }
        }
    }

    Ok(TeamRuntimeControlRecord {
        team_id: team.id.clone(),
        status: TeamRuntimeStatus::Running,
        members,
    })
}

pub async fn force_team_member_new_session(
    agents: &AgentManager,
    team: &TeamDefinitionRecord,
    member_id: &str,
) -> anyhow::Result<TeamRuntimeControlRecord> {
    let member_specs = parse_runtime_member_specs(&team.spec)?;
    let member = member_specs
        .iter()
        .find(|candidate| candidate.member_id == member_id)
        .ok_or_else(|| {
            TeamRuntimeStartError::InvalidConfig(format!(
                "team member '{}' is not defined in team '{}'",
                member_id, team.id
            ))
        })?;
    reconcile_team_member_runtime(agents, member)
        .await
        .with_context(|| format!("prepare runtime for member '{}'", member.member_id))?;
    let actor_context = build_team_member_actor_context(team.id.as_str(), member)
        .with_context(|| format!("build actor context for member '{}'", member.member_id))?;
    let agent = agents
        .get_agent(member.member_id.as_str())
        .await
        .map_err(|err| map_member_agent_lookup_error(member.member_id.as_str(), err))?;
    let had_live_session = agents
        .live_session_id_for_agent(member.member_id.as_str())
        .await
        .with_context(|| format!("load live session for member '{}'", member.member_id))?
        .is_some();
    let provider = agents
        .acp_provider_for_agent(&agent.command, &agent.args)
        .unwrap_or("codex");
    // Force New Session is the explicit continuity reset path: clear the persisted ACP
    // session first, then restart the member runtime on a fresh provider session.
    agents
        .clear_persistent_session(member.member_id.as_str(), provider)
        .await
        .with_context(|| {
            format!(
                "clear persistent {} session for member '{}'",
                provider, member.member_id
            )
        })?;

    let action = if agents
        .running_session_id_for_agent(member.member_id.as_str())
        .await
        .is_some()
    {
        agents
            .stop_agent(member.member_id.as_str())
            .await
            .with_context(|| format!("stop runtime for member '{}'", member.member_id))?;
        "forced_restart"
    } else if had_live_session {
        "forced_restart"
    } else {
        "forced_new_session"
    };

    let session_id = agents
        .start_agent_with_actor_context(member.member_id.as_str(), Some(actor_context))
        .await
        .map_err(|err| {
            tracing::error!(
                team_id = %team.id,
                member_id = %member.member_id,
                error = %err,
                "failed to force a new runtime session for team member"
            );
            TeamRuntimeStartError::MemberRuntimeStart(format!(
                "failed to force a new session for team member '{}' in team '{}'",
                member.member_id, team.id
            ))
        })?;

    Ok(TeamRuntimeControlRecord {
        team_id: team.id.clone(),
        status: TeamRuntimeStatus::Running,
        members: vec![TeamRuntimeMemberStatusRecord {
            member_id: member.member_id.clone(),
            session_id,
            action: action.to_string(),
        }],
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
