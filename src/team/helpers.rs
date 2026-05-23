use crate::acp::{AcpActorSkillContext, DEFAULT_ACTOR_CHANNEL};
use serde_json::Value;

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
