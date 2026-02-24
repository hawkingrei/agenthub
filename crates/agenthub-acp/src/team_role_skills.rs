use agenthub_acp_core::{AcpSkill, build_skill};

use crate::AcpActorSkillContext;

const TEAM_LEADER_SKILL_TEXT: &str =
    include_str!("../../../skills/team/team-leader-orchestrator.SKILL.md");
const TEAM_WORKER_SKILL_TEXT: &str =
    include_str!("../../../skills/team/team-worker-executor.SKILL.md");
const TEAM_DELIBERATION_SKILL_TEXT: &str =
    include_str!("../../../skills/team/team-deliberation-rules.SKILL.md");
const TEAM_ACTOR_MAILBOX_SKILL_TEXT: &str =
    include_str!("../../../skills/team/team-actor-mailbox.SKILL.md");
const TEAM_AGENTS_INDEX_SKILL_TEXT: &str =
    include_str!("../../../skills/team/team-agents-index.SKILL.md");
const TEAM_LEADER_AGENTS_INDEX_SKILL_TEXT: &str =
    include_str!("../../../skills/team/team-leader-agents-index.SKILL.md");
const TEAM_WORKER_AGENTS_INDEX_SKILL_TEXT: &str =
    include_str!("../../../skills/team/team-worker-agents-index.SKILL.md");

const TEAM_AGENTS_INDEX_SKILL_NAME: &str = "team-agents-index";
const TEAM_LEADER_AGENTS_INDEX_SKILL_NAME: &str = "team-leader-agents-index";
const TEAM_WORKER_AGENTS_INDEX_SKILL_NAME: &str = "team-worker-agents-index";
const TEAM_LEADER_SKILL_NAME: &str = "team-leader-orchestrator";
const TEAM_WORKER_SKILL_NAME: &str = "team-worker-executor";
const TEAM_DELIBERATION_SKILL_NAME: &str = "team-deliberation-rules";
const TEAM_ACTOR_MAILBOX_SKILL_NAME: &str = "team-actor-mailbox";

fn normalize_member_role(role: Option<&str>) -> Option<&str> {
    role.map(str::trim).filter(|value| !value.is_empty())
}

pub(super) fn should_attach_team_role_skills(context: Option<&AcpActorSkillContext>) -> bool {
    matches!(
        context.and_then(|item| normalize_member_role(item.member_role.as_deref())),
        Some("leader" | "worker")
    )
}

pub(super) fn is_reserved_team_role_skill(name: &str) -> bool {
    name.eq_ignore_ascii_case(TEAM_AGENTS_INDEX_SKILL_NAME)
        || name.eq_ignore_ascii_case(TEAM_LEADER_AGENTS_INDEX_SKILL_NAME)
        || name.eq_ignore_ascii_case(TEAM_WORKER_AGENTS_INDEX_SKILL_NAME)
        || name.eq_ignore_ascii_case(TEAM_LEADER_SKILL_NAME)
        || name.eq_ignore_ascii_case(TEAM_WORKER_SKILL_NAME)
        || name.eq_ignore_ascii_case(TEAM_DELIBERATION_SKILL_NAME)
        || name.eq_ignore_ascii_case(TEAM_ACTOR_MAILBOX_SKILL_NAME)
}

pub(super) fn build_team_role_skills(context: &AcpActorSkillContext) -> Vec<AcpSkill> {
    let role = normalize_member_role(context.member_role.as_deref());
    let mut out = Vec::new();
    match role {
        Some("leader") => {
            out.push(build_skill(
                TEAM_AGENTS_INDEX_SKILL_NAME.to_string(),
                "builtin://agenthub/team/team-agents-index".to_string(),
                TEAM_AGENTS_INDEX_SKILL_TEXT,
            ));
            out.push(build_skill(
                TEAM_LEADER_AGENTS_INDEX_SKILL_NAME.to_string(),
                "builtin://agenthub/team/team-leader-agents-index".to_string(),
                TEAM_LEADER_AGENTS_INDEX_SKILL_TEXT,
            ));
            out.push(build_skill(
                TEAM_LEADER_SKILL_NAME.to_string(),
                "builtin://agenthub/team/team-leader-orchestrator".to_string(),
                TEAM_LEADER_SKILL_TEXT,
            ));
            out.push(build_skill(
                TEAM_ACTOR_MAILBOX_SKILL_NAME.to_string(),
                "builtin://agenthub/team/team-actor-mailbox".to_string(),
                TEAM_ACTOR_MAILBOX_SKILL_TEXT,
            ));
            out.push(build_skill(
                TEAM_DELIBERATION_SKILL_NAME.to_string(),
                "builtin://agenthub/team/team-deliberation-rules".to_string(),
                TEAM_DELIBERATION_SKILL_TEXT,
            ));
        }
        Some("worker") => {
            out.push(build_skill(
                TEAM_AGENTS_INDEX_SKILL_NAME.to_string(),
                "builtin://agenthub/team/team-agents-index".to_string(),
                TEAM_AGENTS_INDEX_SKILL_TEXT,
            ));
            out.push(build_skill(
                TEAM_WORKER_AGENTS_INDEX_SKILL_NAME.to_string(),
                "builtin://agenthub/team/team-worker-agents-index".to_string(),
                TEAM_WORKER_AGENTS_INDEX_SKILL_TEXT,
            ));
            out.push(build_skill(
                TEAM_WORKER_SKILL_NAME.to_string(),
                "builtin://agenthub/team/team-worker-executor".to_string(),
                TEAM_WORKER_SKILL_TEXT,
            ));
            out.push(build_skill(
                TEAM_ACTOR_MAILBOX_SKILL_NAME.to_string(),
                "builtin://agenthub/team/team-actor-mailbox".to_string(),
                TEAM_ACTOR_MAILBOX_SKILL_TEXT,
            ));
            out.push(build_skill(
                TEAM_DELIBERATION_SKILL_NAME.to_string(),
                "builtin://agenthub/team/team-deliberation-rules".to_string(),
                TEAM_DELIBERATION_SKILL_TEXT,
            ));
        }
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        build_team_role_skills, is_reserved_team_role_skill, should_attach_team_role_skills,
    };
    use crate::AcpActorSkillContext;

    fn context_with_role(role: Option<&str>) -> AcpActorSkillContext {
        AcpActorSkillContext {
            run_id: "run-1".to_string(),
            actor_id: "actor-1".to_string(),
            default_channel: "default".to_string(),
            actor_cli_path: "/tmp/agenthub".to_string(),
            member_role: role.map(str::to_string),
            continuity: None,
        }
    }

    #[test]
    fn build_team_role_skills_for_leader() {
        let skills = build_team_role_skills(&context_with_role(Some("leader")));
        let names = skills
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "team-agents-index",
                "team-leader-agents-index",
                "team-leader-orchestrator",
                "team-actor-mailbox",
                "team-deliberation-rules"
            ]
        );
    }

    #[test]
    fn build_team_role_skills_for_worker() {
        let skills = build_team_role_skills(&context_with_role(Some("worker")));
        let names = skills
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "team-agents-index",
                "team-worker-agents-index",
                "team-worker-executor",
                "team-actor-mailbox",
                "team-deliberation-rules"
            ]
        );
    }

    #[test]
    fn build_team_role_skills_skips_unknown_role() {
        assert!(build_team_role_skills(&context_with_role(Some("observer"))).is_empty());
        assert!(build_team_role_skills(&context_with_role(None)).is_empty());
    }

    #[test]
    fn should_attach_team_role_skills_checks_supported_roles() {
        assert!(should_attach_team_role_skills(Some(&context_with_role(
            Some("leader")
        ))));
        assert!(should_attach_team_role_skills(Some(&context_with_role(
            Some("worker")
        ))));
        assert!(!should_attach_team_role_skills(Some(&context_with_role(
            Some("observer")
        ))));
        assert!(!should_attach_team_role_skills(Some(&context_with_role(
            None
        ))));
        assert!(!should_attach_team_role_skills(None));
    }

    #[test]
    fn is_reserved_team_role_skill_matches_expected_names() {
        assert!(is_reserved_team_role_skill("team-agents-index"));
        assert!(is_reserved_team_role_skill("team-leader-agents-index"));
        assert!(is_reserved_team_role_skill("team-worker-agents-index"));
        assert!(is_reserved_team_role_skill("team-leader-orchestrator"));
        assert!(is_reserved_team_role_skill("team-worker-executor"));
        assert!(is_reserved_team_role_skill("team-deliberation-rules"));
        assert!(is_reserved_team_role_skill("team-actor-mailbox"));
        assert!(is_reserved_team_role_skill("TEAM-WORKER-EXECUTOR"));
        assert!(!is_reserved_team_role_skill("custom-skill"));
    }
}
