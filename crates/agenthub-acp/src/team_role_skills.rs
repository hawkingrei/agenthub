use std::path::Path;

use agenthub_acp_core::AcpSkill;
use agenthub_managed_skills::ManagedSkillKind;
use anyhow::Result;

use crate::AcpActorSkillContext;
use crate::actor_runtime_skill::build_required_managed_skill;

const TEAM_AGENTS_INDEX_SKILL_NAME: &str = "team-agents-index";
const TEAM_COORDINATOR_AGENTS_INDEX_SKILL_NAME: &str = "team-coordinator-agents-index";
const TEAM_WORKER_AGENTS_INDEX_SKILL_NAME: &str = "team-worker-agents-index";
const TEAM_COORDINATOR_SKILL_NAME: &str = "team-coordinator-orchestrator";
const TEAM_WORKER_SKILL_NAME: &str = "team-worker-executor";
const TEAM_ACTOR_MAILBOX_SKILL_NAME: &str = "team-actor-mailbox";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TeamRoleIndexKind {
    Coordinator,
    Worker,
}

impl TeamRoleIndexKind {
    fn managed_kind(self) -> ManagedSkillKind {
        match self {
            Self::Coordinator => ManagedSkillKind::TeamCoordinatorAgentsIndex,
            Self::Worker => ManagedSkillKind::TeamWorkerAgentsIndex,
        }
    }
}

fn build_role_agents_index_skill(
    kind: TeamRoleIndexKind,
    home_dir: Option<&Path>,
) -> Result<AcpSkill> {
    build_required_managed_skill(kind.managed_kind(), home_dir)
}

fn normalize_member_role(role: Option<&str>) -> Option<&str> {
    role.map(str::trim).filter(|value| !value.is_empty())
}

pub(super) fn should_attach_team_role_skills(context: Option<&AcpActorSkillContext>) -> bool {
    matches!(
        context.and_then(|item| normalize_member_role(item.member_role.as_deref())),
        Some("coordinator" | "worker")
    )
}

pub(super) fn is_reserved_team_role_skill(name: &str) -> bool {
    name.eq_ignore_ascii_case(TEAM_AGENTS_INDEX_SKILL_NAME)
        || name.eq_ignore_ascii_case(TEAM_COORDINATOR_AGENTS_INDEX_SKILL_NAME)
        || name.eq_ignore_ascii_case(TEAM_WORKER_AGENTS_INDEX_SKILL_NAME)
        || name.eq_ignore_ascii_case(TEAM_COORDINATOR_SKILL_NAME)
        || name.eq_ignore_ascii_case(TEAM_WORKER_SKILL_NAME)
        || name.eq_ignore_ascii_case(TEAM_ACTOR_MAILBOX_SKILL_NAME)
}

pub(super) fn build_team_role_skills(context: &AcpActorSkillContext) -> Result<Vec<AcpSkill>> {
    build_team_role_skills_with_home(context, None)
}

fn build_team_role_skills_with_home(
    context: &AcpActorSkillContext,
    home_dir: Option<&Path>,
) -> Result<Vec<AcpSkill>> {
    let role = normalize_member_role(context.member_role.as_deref());
    let mut out = Vec::new();
    match role {
        Some("coordinator") => {
            out.push(build_required_managed_skill(
                ManagedSkillKind::TeamAgentsIndex,
                home_dir,
            )?);
            out.push(build_role_agents_index_skill(
                TeamRoleIndexKind::Coordinator,
                home_dir,
            )?);
            out.push(build_required_managed_skill(
                ManagedSkillKind::TeamCoordinatorOrchestrator,
                home_dir,
            )?);
            out.push(build_required_managed_skill(
                ManagedSkillKind::TeamActorMailbox,
                home_dir,
            )?);
        }
        Some("worker") => {
            out.push(build_required_managed_skill(
                ManagedSkillKind::TeamAgentsIndex,
                home_dir,
            )?);
            out.push(build_role_agents_index_skill(
                TeamRoleIndexKind::Worker,
                home_dir,
            )?);
            out.push(build_required_managed_skill(
                ManagedSkillKind::TeamWorkerExecutor,
                home_dir,
            )?);
            out.push(build_required_managed_skill(
                ManagedSkillKind::TeamActorMailbox,
                home_dir,
            )?);
        }
        _ => {}
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use agenthub_managed_skills::install_managed_skills;

    use super::{
        TeamRoleIndexKind, build_role_agents_index_skill, build_team_role_skills,
        build_team_role_skills_with_home, is_reserved_team_role_skill,
        should_attach_team_role_skills,
    };
    use crate::AcpActorSkillContext;
    use crate::test_utils::TempManagedSkillsHome;

    fn context_with_role(role: Option<&str>) -> AcpActorSkillContext {
        AcpActorSkillContext {
            team_id: Some("team-1".to_string()),
            current_run_id: Some("run-1".to_string()),
            actor_id: "actor-1".to_string(),
            default_channel: "default".to_string(),
            member_role: role.map(str::to_string),
            member_skills: Vec::new(),
            contract_version: None,
            continuity: None,
        }
    }

    #[test]
    fn build_team_role_skills_for_coordinator() {
        let home = TempManagedSkillsHome::new("agenthub-acp-team-role-skill-home");
        install_managed_skills(Some(home.path())).expect("install managed skills");
        let skills = build_team_role_skills_with_home(
            &context_with_role(Some("coordinator")),
            Some(home.path()),
        )
        .expect("build coordinator team role skills");
        let names = skills
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "team-agents-index",
                "team-coordinator-agents-index",
                "team-coordinator-orchestrator",
                "team-actor-mailbox",
            ]
        );
    }

    #[test]
    fn build_team_role_skills_for_worker() {
        let home = TempManagedSkillsHome::new("agenthub-acp-team-role-skill-home");
        install_managed_skills(Some(home.path())).expect("install managed skills");
        let skills =
            build_team_role_skills_with_home(&context_with_role(Some("worker")), Some(home.path()))
                .expect("build worker team role skills");
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
    fn build_team_role_skills_skips_unknown_role() {
        assert!(
            build_team_role_skills(&context_with_role(Some("observer")))
                .expect("build observer team role skills")
                .is_empty()
        );
        assert!(
            build_team_role_skills(&context_with_role(None))
                .expect("build empty team role skills")
                .is_empty()
        );
    }

    #[test]
    fn should_attach_team_role_skills_checks_supported_roles() {
        assert!(should_attach_team_role_skills(Some(&context_with_role(
            Some("coordinator")
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
        assert!(is_reserved_team_role_skill("team-coordinator-agents-index"));
        assert!(is_reserved_team_role_skill("team-worker-agents-index"));
        assert!(is_reserved_team_role_skill("team-coordinator-orchestrator"));
        assert!(is_reserved_team_role_skill("team-worker-executor"));
        assert!(is_reserved_team_role_skill("team-actor-mailbox"));
        assert!(is_reserved_team_role_skill("TEAM-WORKER-EXECUTOR"));
        assert!(!is_reserved_team_role_skill("team-task-lifecycle"));
        assert!(!is_reserved_team_role_skill("team-deliberation-rules"));
        assert!(!is_reserved_team_role_skill("custom-skill"));
    }

    #[test]
    fn role_agents_index_skill_uses_expected_names() {
        let home = TempManagedSkillsHome::new("agenthub-acp-team-role-skill-home");
        install_managed_skills(Some(home.path())).expect("install managed skills");
        let coordinator =
            build_role_agents_index_skill(TeamRoleIndexKind::Coordinator, Some(home.path()))
                .expect("build coordinator role agents index skill");
        assert_eq!(coordinator.name, "team-coordinator-agents-index");

        let worker = build_role_agents_index_skill(TeamRoleIndexKind::Worker, Some(home.path()))
            .expect("build worker role agents index skill");
        assert_eq!(worker.name, "team-worker-agents-index");
    }

    #[test]
    fn build_team_role_skills_error_when_managed_skill_not_materialized() {
        let home = TempManagedSkillsHome::new("agenthub-acp-team-role-skill-home");
        let err = build_team_role_skills_with_home(
            &context_with_role(Some("coordinator")),
            Some(home.path()),
        )
        .expect_err("missing managed team skills should hard fail");
        assert!(
            err.to_string().contains("is not materialized"),
            "unexpected error: {err}"
        );
        assert!(
            err.to_string().contains("ACP session"),
            "unexpected error: {err}"
        );
    }
}
