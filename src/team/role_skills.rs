const COORDINATOR_ROLE_SYSTEM_SKILLS: [&str; 5] = [
    "agenthub-actor-runtime",
    "team-agents-index",
    "team-coordinator-agents-index",
    "team-coordinator-orchestrator",
    "team-actor-mailbox",
];

const WORKER_ROLE_SYSTEM_SKILLS: [&str; 5] = [
    "agenthub-actor-runtime",
    "team-agents-index",
    "team-worker-agents-index",
    "team-worker-executor",
    "team-actor-mailbox",
];

pub fn mandatory_team_role_skills(role: &str) -> &'static [&'static str] {
    if role == "coordinator" {
        COORDINATOR_ROLE_SYSTEM_SKILLS.as_slice()
    } else {
        WORKER_ROLE_SYSTEM_SKILLS.as_slice()
    }
}

pub fn effective_team_member_skills(role: &str) -> Vec<String> {
    mandatory_team_role_skills(role)
        .iter()
        .map(|skill| (*skill).to_string())
        .collect()
}
