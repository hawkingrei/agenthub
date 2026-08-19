use anyhow::Context;
use serde_json::Value;

use crate::acp::AcpActorSkillContext;
use crate::team::build_team_member_actor_context_for_role;

#[derive(Debug, Clone)]
pub(super) struct TeamRuntimeMemberSpec {
    pub member_id: String,
    pub role: String,
    pub runtime: Option<TeamRuntimeMemberRuntimeHint>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub(super) struct TeamRuntimeMemberRuntimeHint {
    #[allow(dead_code)]
    pub name: Option<String>,
    pub target_node_id: Option<String>,
    pub workdir: Option<String>,
    pub worktree_repo: Option<String>,
    pub worktree_ref: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub agent_loop_enabled: Option<bool>,
    #[serde(default)]
    #[allow(dead_code)]
    pub agent_loop_idle_seconds: Option<i64>,
    #[serde(default)]
    #[allow(dead_code)]
    pub agent_loop_prompt: Option<String>,
}

pub(super) fn parse_runtime_member_specs(
    spec: &Value,
) -> anyhow::Result<Vec<TeamRuntimeMemberSpec>> {
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
        let runtime = member_obj
            .get("runtime")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .context("parse spec.members[].runtime")?;
        out.push(TeamRuntimeMemberSpec {
            member_id: member_id.to_string(),
            role: role.to_string(),
            runtime,
        });
    }
    Ok(out)
}

pub(super) fn build_team_member_actor_context(
    team_id: &str,
    member: &TeamRuntimeMemberSpec,
) -> anyhow::Result<AcpActorSkillContext> {
    Ok(build_team_member_actor_context_for_role(
        team_id,
        None,
        &member.member_id,
        &member.role,
    ))
}

pub(super) fn team_member_actor_context_matches(
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

pub(super) fn trimmed_opt(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
