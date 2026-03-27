use std::collections::HashSet;

const LEADER_ROLE_SYSTEM_SKILLS: [&str; 5] = [
    "agenthub-actor-runtime",
    "team-agents-index",
    "team-leader-agents-index",
    "team-leader-orchestrator",
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
    if role == "leader" {
        LEADER_ROLE_SYSTEM_SKILLS.as_slice()
    } else {
        WORKER_ROLE_SYSTEM_SKILLS.as_slice()
    }
}

pub fn effective_team_member_skills(role: &str) -> Vec<String> {
    let mut out = Vec::with_capacity(mandatory_team_role_skills(role).len());
    let mut seen = HashSet::with_capacity(out.capacity());
    for skill in mandatory_team_role_skills(role) {
        if seen.insert((*skill).to_string()) {
            out.push((*skill).to_string());
        }
    }
    out
}
