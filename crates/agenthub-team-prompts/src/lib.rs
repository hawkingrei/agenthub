pub const DEFAULT_TEAM_COORDINATOR_PROMPT: &str =
    include_str!("../prompts/default_team_coordinator_prompt.txt");

pub const DEFAULT_TEAM_WORKER_PROMPT: &str =
    include_str!("../prompts/default_team_worker_prompt.txt");

pub fn default_team_prompt_for_role(role: &str) -> &'static str {
    match role {
        "coordinator" => DEFAULT_TEAM_COORDINATOR_PROMPT,
        _ => DEFAULT_TEAM_WORKER_PROMPT,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_TEAM_COORDINATOR_PROMPT, DEFAULT_TEAM_WORKER_PROMPT, default_team_prompt_for_role,
    };

    fn verify_execution_vocabulary(prompt: &str) {
        assert!(prompt.contains("Treat `task` as the primary ownership object"));
        assert!(prompt.contains("Treat `run` and `step` as execution/debug artifacts"));
        assert!(prompt.contains("explicit later resume starts the next attempt"));
        assert!(prompt.contains("waiting"));
        assert!(prompt.contains("in_review"));
    }

    #[test]
    fn default_prompt_resolves_by_role() {
        assert_eq!(
            default_team_prompt_for_role("coordinator"),
            DEFAULT_TEAM_COORDINATOR_PROMPT
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
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("coordinator_task_assignment"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("worker_status"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("Team Coordinator"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("team coordinator"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("Team workflow phases"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("Team workflow phases"));
        assert!(!DEFAULT_TEAM_COORDINATOR_PROMPT.contains(".cache/context/todo"));
        assert!(!DEFAULT_TEAM_WORKER_PROMPT.contains(".cache/context/todo"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("Decision Complete"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("Think from first principles"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("First-principles reasoning first"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("Explore Before Asking"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("Clearance checklist before delegation"));
        assert!(
            DEFAULT_TEAM_COORDINATOR_PROMPT.contains("volatile runtime state out of prompt prose")
        );
        assert!(
            DEFAULT_TEAM_COORDINATOR_PROMPT
                .contains("current objective, next action, allowed-action gate")
        );
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains(".cache/context/run/<run_id>/..."));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("spec.members[].member_id"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("spec.members[].description"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains(".well-known/agent-card"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("actor team-members"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("actor team-tasks"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("actor team-task-create"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("actor team-task-update"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("must have an explicit assigned member"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("profile_patch_proposal"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("actor time-trigger-set"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("agent_loop"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("least-privilege intent"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("spec.members[].description"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains(".well-known/agent-card"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("actor team-members"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("actor team-tasks"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("actor team-task-create"));
        assert!(
            DEFAULT_TEAM_WORKER_PROMPT.contains("must always have an explicit assigned member")
        );
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("profile_patch_proposal"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("actor time-trigger-set"));
        assert!(
            DEFAULT_TEAM_WORKER_PROMPT.contains("volatile execution detail out of prompt prose")
        );
        assert!(
            DEFAULT_TEAM_WORKER_PROMPT
                .contains("active assignment, next action, allowed-action gate")
        );
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains(".cache/context/run/<run_id>/..."));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("agent_loop"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("least-privilege intent"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("review action in your current session"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("Finalization by mode"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("skills/team/TEAM_AGENTS.md"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("skills/team/TEAM_AGENTS.md"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("team-agents-index"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("team-agents-index"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("team-reporting-surfaces"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("team-reporting-surfaces"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("team-task-governance"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("team-task-governance"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("team-task-lifecycle"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("team-task-lifecycle"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("team-actor-mailbox"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("team-actor-mailbox"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("canonical Team task creation"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("advance assigned tasks"));
        verify_execution_vocabulary(DEFAULT_TEAM_COORDINATOR_PROMPT);
        verify_execution_vocabulary(DEFAULT_TEAM_WORKER_PROMPT);
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("Inspect inbox regularly"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("Receive inbox work"));
        assert!(
            DEFAULT_TEAM_COORDINATOR_PROMPT
                .contains("When a new inbox, channel, or thread message arrives")
        );
        assert!(
            DEFAULT_TEAM_WORKER_PROMPT
                .contains("When a new inbox, channel, or thread message arrives")
        );
        assert!(
            DEFAULT_TEAM_COORDINATOR_PROMPT
                .contains("routing, ack, open-thread, or reply mechanics")
        );
        assert!(
            DEFAULT_TEAM_WORKER_PROMPT.contains("routing, ack, open-thread, or reply mechanics")
        );
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("before replying or mutating task state"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("before responding"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("Think from first principles"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("Re-derive the problem from first principles"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("Treat inbox inspection as read-only"));
        assert!(!DEFAULT_TEAM_COORDINATOR_PROMPT.contains("Pull inbox regularly"));
        assert!(!DEFAULT_TEAM_WORKER_PROMPT.contains("Acknowledge messages after reading"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("direct mailbox first"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("direct mailbox first"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("not a shared Team surface by itself"));
        assert!(
            DEFAULT_TEAM_WORKER_PROMPT
                .contains("not automatically visible to other agents or humans")
        );
        assert!(
            DEFAULT_TEAM_COORDINATOR_PROMPT.contains("Workers should have meaningful initiative")
        );
        assert!(
            DEFAULT_TEAM_WORKER_PROMPT
                .contains("Inside your assigned execution lane, use more initiative by default")
        );
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("active dialogue with the coordinator"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("detail_ref"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("detail_ref"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("Human channel input may be free-form"));
        assert!(
            DEFAULT_TEAM_COORDINATOR_PROMPT
                .contains("visible reply lands back in the original human conversation")
        );
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("turn that into a direct channel reply"));
        assert!(
            DEFAULT_TEAM_COORDINATOR_PROMPT
                .contains("runtime-injected Team identity and recovery pointers as authoritative")
        );
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains(".cache/context/state.md"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("concrete reply target or thread target"));
        assert!(
            DEFAULT_TEAM_COORDINATOR_PROMPT
                .contains("Do not create a new canonical Team task for every quick answer")
        );
        assert!(
            DEFAULT_TEAM_COORDINATOR_PROMPT
                .contains("immediate visible acknowledgment, blocker signal, or ownership signal")
        );
        assert!(
            DEFAULT_TEAM_COORDINATOR_PROMPT.contains("timed triggers for real deferred follow-up")
        );
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("owed blocker or handoff update"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains(".agenthubmemory/"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("does not need `.agenthubmemory/`"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains(".agenthubmemory/TODO.md"));
        assert!(
            DEFAULT_TEAM_WORKER_PROMPT
                .contains("runtime-injected Team identity and recovery pointers as authoritative")
        );
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains(".cache/context/state.md"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("concrete reply target or thread target"));
        assert!(
            DEFAULT_TEAM_WORKER_PROMPT
                .contains("immediate visible acknowledgment, blocker signal, or ownership signal")
        );
        assert!(
            DEFAULT_TEAM_WORKER_PROMPT.contains("answer directly on the original visible surface")
        );
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("timed triggers for real deferred follow-up"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("owed blocker or handoff update"));
        assert!(
            DEFAULT_TEAM_COORDINATOR_PROMPT.contains("Treat `# all` as the default Team channel")
        );
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("Treat `# all` as the default Team channel"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("summary entrypoint for one topic"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("summary entrypoint for one topic"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("full-context container for that topic"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("full-context container for that topic"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("open the thread before assuming"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("open the thread before assuming"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("Prefer thread replies"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("Prefer thread replies"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("agenthub actor team-thread-open"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("agenthub actor team-thread-open"));
        assert!(DEFAULT_TEAM_COORDINATOR_PROMPT.contains("agenthub actor team-thread-reply"));
        assert!(DEFAULT_TEAM_WORKER_PROMPT.contains("agenthub actor team-thread-reply"));
        assert!(
            DEFAULT_TEAM_WORKER_PROMPT.contains("answer directly in the relevant Team channel")
        );
    }
}
