use agenthub_acp::AcpActorSkillContext;

pub(crate) const DEFAULT_ACTOR_CHANNEL: &str = "default";

pub(crate) fn normalize_actor_context(
    context: AcpActorSkillContext,
) -> anyhow::Result<AcpActorSkillContext> {
    let default_channel = if context.default_channel.trim().is_empty() {
        DEFAULT_ACTOR_CHANNEL.to_string()
    } else {
        context.default_channel.trim().to_string()
    };
    let member_role = context
        .member_role
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());
    let mut member_skills = Vec::with_capacity(context.member_skills.len());
    for skill in context.member_skills {
        let normalized = skill.trim();
        if normalized.is_empty() {
            continue;
        }
        if !member_skills
            .iter()
            .any(|item: &String| item.eq_ignore_ascii_case(normalized))
        {
            member_skills.push(normalized.to_string());
        }
    }
    Ok(AcpActorSkillContext {
        team_id: context
            .team_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        current_run_id: context
            .current_run_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        actor_id: context.actor_id.trim().to_string(),
        default_channel,
        member_role,
        member_skills,
        contract_version: context
            .contract_version
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        continuity: context.continuity,
    })
}

#[cfg(test)]
mod tests {
    use super::DEFAULT_ACTOR_CHANNEL;

    #[test]
    fn default_channel_constant_is_stable() {
        assert_eq!(DEFAULT_ACTOR_CHANNEL, "default");
    }
}
