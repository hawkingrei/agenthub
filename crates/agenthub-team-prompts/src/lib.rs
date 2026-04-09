pub const DEFAULT_TEAM_LEADER_PROMPT: &str =
    include_str!("../prompts/default_team_leader_prompt.txt");

pub const DEFAULT_TEAM_WORKER_PROMPT: &str =
    include_str!("../prompts/default_team_worker_prompt.txt");

pub fn default_team_prompt_for_role(role: &str) -> &'static str {
    match role {
        "leader" => DEFAULT_TEAM_LEADER_PROMPT,
        _ => DEFAULT_TEAM_WORKER_PROMPT,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_TEAM_LEADER_PROMPT, DEFAULT_TEAM_WORKER_PROMPT, default_team_prompt_for_role,
    };

    #[test]
    fn default_prompt_resolves_by_role() {
        assert_eq!(
            default_team_prompt_for_role("leader"),
            DEFAULT_TEAM_LEADER_PROMPT
        );
        assert_eq!(
            default_team_prompt_for_role("worker"),
            DEFAULT_TEAM_WORKER_PROMPT
        );
        assert_eq!(
            default_team_prompt_for_role("unknown"),
            DEFAULT_TEAM_WORKER_PROMPT
        );
    }

    #[test]
    fn prompt_templates_keep_required_contract_lines() {
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("leader_task_assignment"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("worker_status"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("Team workflow phases"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("Team workflow phases"));
        assert!(!DEFAULT_TEAM_LEADER_PROMPT.contains(".cache/context/todo"));
        assert!(!DEFAULT_TEAM_WORKER_PROMPT.contains(".cache/context/todo"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("Decision Complete"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("Think from first principles"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("First-principles reasoning first"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("Explore Before Asking"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("Clearance checklist before delegation"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("spec.members[].member_id"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("spec.members[].description"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains(".well-known/agent-card"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("actor team-members"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("actor team-tasks"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("actor team-task-create"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("actor team-task-update"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("profile_patch_proposal"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("actor time-trigger-set"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("agent_loop"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("least-privilege intent"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("spec.members[].description"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains(".well-known/agent-card"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("actor team-members"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("actor team-tasks"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("actor team-task-create"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("profile_patch_proposal"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("actor time-trigger-set"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("agent_loop"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("least-privilege intent"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("review action in your current session"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("Finalization by mode"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("skills/team/TEAM_AGENTS.md"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("skills/team/TEAM_AGENTS.md"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("team-agents-index"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("team-agents-index"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("team-task-lifecycle"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("team-task-lifecycle"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("canonical Team task creation"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("advance assigned tasks"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("waiting"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("waiting"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("in_review"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("in_review"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("Inspect inbox regularly"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("Receive inbox work"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("Think from first principles"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("Re-derive the problem from first principles"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("Treat inbox inspection as read-only"));
        assert!(!DEFAULT_TEAM_LEADER_PROMPT.contains("Pull inbox regularly"));
        assert!(!DEFAULT_TEAM_WORKER_PROMPT.contains("Acknowledge messages after reading"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("direct mailbox first"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("direct mailbox first"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("detail_ref"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("detail_ref"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("Human channel input may be free-form"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains(".agenthubmemory/"));
        assert!(DEFAULT_TEAM_LEADER_PROMPT.contains("does not need `.agenthubmemory/`"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains(".agenthubmemory/TODO.md"));
    }
}
