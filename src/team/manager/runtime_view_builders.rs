use serde_json::Value;

use super::runtime_view_loaders::AgentRuntimeRow;
use super::runtime_views::TeamMemberSpecView;
use super::{
    TeamManager, TeamMemberCardRecord, TeamRunMemberRecord, TeamRuntimeMemberRecord,
    TeamRuntimeRecord, TeamRuntimeSummaryRecord,
};

impl TeamManager {
    /// Use the same Card builder as discovery, with the configuration already pinned for launch.
    pub(crate) fn member_card_for_launch(
        spec: &Value,
        agent: &crate::agent::AgentRecord,
    ) -> anyhow::Result<TeamMemberCardRecord> {
        let member = parse_team_member_specs(spec)?
            .into_iter()
            .find(|member| member.member_id == agent.id)
            .ok_or_else(|| anyhow::anyhow!("loop member Card is unavailable"))?;
        let worktree_mode = serde_json::to_value(&agent.worktree_mode)?;
        let runtime = AgentRuntimeRow {
            name: agent.name.clone(),
            status: None,
            code_mode: agent.code_mode,
            worktree_mode: worktree_mode.as_str().map(str::to_owned),
        };
        Ok(build_team_member_card(&member, Some(&runtime), &agent.name))
    }
}

pub(super) fn parse_team_member_specs(spec: &Value) -> anyhow::Result<Vec<TeamMemberSpecView>> {
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
        let description = member_obj
            .get("description")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        out.push(TeamMemberSpecView {
            member_id: member_id.to_string(),
            role: role.to_string(),
            description,
        });
    }
    Ok(out)
}

pub(super) fn build_team_member_card(
    member: &TeamMemberSpecView,
    agent: Option<&AgentRuntimeRow>,
    display_name: &str,
) -> TeamMemberCardRecord {
    let mut capability_tags = vec![
        "team_mailbox_v1".to_string(),
        "team_step_execution_v1".to_string(),
    ];
    if let Some(agent) = agent {
        if agent.code_mode {
            capability_tags.push("code_mode".to_string());
        }
        if let Some(worktree_mode) = agent
            .worktree_mode
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            && matches!(worktree_mode, "create_worktree" | "reuse_worktree")
        {
            capability_tags.push("git_worktree".to_string());
        }
    }
    let description = member.description.clone().unwrap_or_else(|| {
        format!(
            "AgentHub team member {} ({}) supports {}",
            display_name,
            member.role,
            capability_tags.join(", ")
        )
    });
    TeamMemberCardRecord {
        card_id: format!("agenthub://team-members/{}", member.member_id),
        schema_version: "agenthub.a2a.discovery_card.v1".to_string(),
        description,
        role: member.role.clone(),
        skills: crate::team::effective_team_member_skills(&member.role),
        capability_tags,
    }
}

pub(super) fn build_team_runtime_summary(runtime: &TeamRuntimeRecord) -> TeamRuntimeSummaryRecord {
    TeamRuntimeSummaryRecord {
        status: runtime.status,
        online_count: runtime
            .members
            .iter()
            .filter(|member| member.session_id.is_some())
            .count(),
        member_count: runtime.members.len(),
    }
}

pub(super) fn team_run_member_from_runtime_member(
    member: TeamRuntimeMemberRecord,
) -> TeamRunMemberRecord {
    TeamRunMemberRecord {
        member_id: member.member_id,
        display_name: member.display_name,
        role: member.role,
        description: member.description,
        pending_inbox_count: member.pending_inbox_count,
        agent_status: member.agent_status,
        session_id: member.session_id,
        session_status: member.session_status,
        card: member.card,
        steps: Vec::new(),
    }
}
