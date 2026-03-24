use agenthub_acp_core::{AcpSkill, build_skill};
use agenthub_managed_skills::{
    ManagedSkillKind, managed_skill_contents, managed_skill_doc, managed_skill_name,
};

use crate::AcpActorSkillContext;

const TEAM_AGENTS_INDEX_SKILL_NAME: &str = "team-agents-index";
const TEAM_LEADER_AGENTS_INDEX_SKILL_NAME: &str = "team-leader-agents-index";
const TEAM_WORKER_AGENTS_INDEX_SKILL_NAME: &str = "team-worker-agents-index";
const TEAM_TASK_LIFECYCLE_SKILL_NAME: &str = "team-task-lifecycle";
const TEAM_LEADER_SKILL_NAME: &str = "team-leader-orchestrator";
const TEAM_WORKER_SKILL_NAME: &str = "team-worker-executor";
const TEAM_DELIBERATION_SKILL_NAME: &str = "team-deliberation-rules";
const TEAM_ACTOR_MAILBOX_SKILL_NAME: &str = "team-actor-mailbox";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TeamRoleIndexKind {
    Leader,
    Worker,
}

impl TeamRoleIndexKind {
    fn managed_kind(self) -> ManagedSkillKind {
        match self {
            Self::Leader => ManagedSkillKind::TeamLeaderAgentsIndex,
            Self::Worker => ManagedSkillKind::TeamWorkerAgentsIndex,
        }
    }

    fn fallback_path(self) -> &'static str {
        match self {
            Self::Leader => "builtin://agenthub/team/team-leader-agents-index",
            Self::Worker => "builtin://agenthub/team/team-worker-agents-index",
        }
    }
}

fn build_managed_skill(kind: ManagedSkillKind, fallback_path: &str) -> AcpSkill {
    match managed_skill_doc(kind, None) {
        Ok(doc) if doc.path.exists() => build_skill(
            doc.name.to_string(),
            doc.path.to_string_lossy().to_string(),
            &doc.contents,
        ),
        Ok(doc) => build_skill(
            doc.name.to_string(),
            fallback_path.to_string(),
            &doc.contents,
        ),
        Err(_) => build_skill(
            managed_skill_name(kind).to_string(),
            fallback_path.to_string(),
            managed_skill_contents(kind).as_str(),
        ),
    }
}

fn build_role_agents_index_skill(kind: TeamRoleIndexKind) -> AcpSkill {
    build_managed_skill(kind.managed_kind(), kind.fallback_path())
}

fn normalize_member_role(role: Option<&str>) -> Option<&str> {
    role.map(str::trim).filter(|value| !value.is_empty())
}

fn has_member_skill(context: &AcpActorSkillContext, skill_name: &str) -> bool {
    context
        .member_skills
        .iter()
        .any(|skill| skill.eq_ignore_ascii_case(skill_name))
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
        || name.eq_ignore_ascii_case(TEAM_TASK_LIFECYCLE_SKILL_NAME)
        || name.eq_ignore_ascii_case(TEAM_LEADER_SKILL_NAME)
        || name.eq_ignore_ascii_case(TEAM_WORKER_SKILL_NAME)
        || name.eq_ignore_ascii_case(TEAM_DELIBERATION_SKILL_NAME)
        || name.eq_ignore_ascii_case(TEAM_ACTOR_MAILBOX_SKILL_NAME)
}

pub(super) fn build_team_role_skills(context: &AcpActorSkillContext) -> Vec<AcpSkill> {
    let role = normalize_member_role(context.member_role.as_deref());
    let enable_deliberation = has_member_skill(context, TEAM_DELIBERATION_SKILL_NAME);
    let enable_task_lifecycle = has_member_skill(context, TEAM_TASK_LIFECYCLE_SKILL_NAME);
    let mut out = Vec::new();
    match role {
        Some("leader") => {
            out.push(build_managed_skill(
                ManagedSkillKind::TeamAgentsIndex,
                "builtin://agenthub/team/team-agents-index",
            ));
            out.push(build_role_agents_index_skill(TeamRoleIndexKind::Leader));
            out.push(build_managed_skill(
                ManagedSkillKind::TeamLeaderOrchestrator,
                "builtin://agenthub/team/team-leader-orchestrator",
            ));
            if enable_task_lifecycle {
                out.push(build_managed_skill(
                    ManagedSkillKind::TeamTaskLifecycle,
                    "builtin://agenthub/team/team-task-lifecycle",
                ));
            }
            out.push(build_managed_skill(
                ManagedSkillKind::TeamActorMailbox,
                "builtin://agenthub/team/team-actor-mailbox",
            ));
            if enable_deliberation {
                out.push(build_managed_skill(
                    ManagedSkillKind::TeamDeliberationRules,
                    "builtin://agenthub/team/team-deliberation-rules",
                ));
            }
        }
        Some("worker") => {
            out.push(build_managed_skill(
                ManagedSkillKind::TeamAgentsIndex,
                "builtin://agenthub/team/team-agents-index",
            ));
            out.push(build_role_agents_index_skill(TeamRoleIndexKind::Worker));
            out.push(build_managed_skill(
                ManagedSkillKind::TeamWorkerExecutor,
                "builtin://agenthub/team/team-worker-executor",
            ));
            if enable_task_lifecycle {
                out.push(build_managed_skill(
                    ManagedSkillKind::TeamTaskLifecycle,
                    "builtin://agenthub/team/team-task-lifecycle",
                ));
            }
            out.push(build_managed_skill(
                ManagedSkillKind::TeamActorMailbox,
                "builtin://agenthub/team/team-actor-mailbox",
            ));
            if enable_deliberation {
                out.push(build_managed_skill(
                    ManagedSkillKind::TeamDeliberationRules,
                    "builtin://agenthub/team/team-deliberation-rules",
                ));
            }
        }
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        TeamRoleIndexKind, build_role_agents_index_skill, build_team_role_skills,
        is_reserved_team_role_skill, should_attach_team_role_skills,
    };
    use crate::AcpActorSkillContext;

    fn context_with_role(role: Option<&str>) -> AcpActorSkillContext {
        AcpActorSkillContext {
            team_id: Some("team-1".to_string()),
            current_run_id: Some("run-1".to_string()),
            actor_id: "actor-1".to_string(),
            default_channel: "default".to_string(),
            actor_cli_path: "/tmp/agenthub".to_string(),
            member_role: role.map(str::to_string),
            member_skills: Vec::new(),
            contract_version: None,
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
            ]
        );
    }

    #[test]
    fn build_team_role_skills_includes_task_lifecycle_when_requested() {
        let mut context = context_with_role(Some("leader"));
        context
            .member_skills
            .push("team-task-lifecycle".to_string());
        let skills = build_team_role_skills(&context);
        let names = skills
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"team-task-lifecycle"));
    }

    #[test]
    fn build_team_role_skills_enables_deliberation_when_requested() {
        let mut context = context_with_role(Some("worker"));
        context
            .member_skills
            .push("team-deliberation-rules".to_string());
        let skills = build_team_role_skills(&context);
        let names = skills
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"team-deliberation-rules"));
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
        assert!(is_reserved_team_role_skill("team-task-lifecycle"));
        assert!(is_reserved_team_role_skill("team-leader-orchestrator"));
        assert!(is_reserved_team_role_skill("team-worker-executor"));
        assert!(is_reserved_team_role_skill("team-deliberation-rules"));
        assert!(is_reserved_team_role_skill("team-actor-mailbox"));
        assert!(is_reserved_team_role_skill("TEAM-WORKER-EXECUTOR"));
        assert!(!is_reserved_team_role_skill("custom-skill"));
    }

    #[test]
    fn role_agents_index_skill_uses_expected_names() {
        let leader = build_role_agents_index_skill(TeamRoleIndexKind::Leader);
        assert_eq!(leader.name, "team-leader-agents-index");

        let worker = build_role_agents_index_skill(TeamRoleIndexKind::Worker);
        assert_eq!(worker.name, "team-worker-agents-index");
    }
}
