use agenthub_acp_core::{AcpSkill, build_skill};
use agenthub_text::truncate_chars;

use crate::AcpActorSkillContext;

pub(super) fn build_actor_runtime_skill(context: &AcpActorSkillContext) -> AcpSkill {
    let continuity_section = build_continuity_section(context);
    let team_section = context
        .team_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("- `team_id`: `{value}`\n"))
        .unwrap_or_default();
    let current_run_section = context
        .current_run_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("- `current_run_id`: `{value}`\n"))
        .unwrap_or_else(|| "- `current_run_id`: `n/a`\n".to_string());
    let instructions = format!(
        r#"# AgentHub Actor Runtime Skill

You are running inside an AgentHub actor session.

{team_section}{current_run_section}- session scope: Team member runtime
- `actor_id`: `{actor_id}`
- `default_channel`: `{default_channel}`
{continuity_section}

Use MCP native actor mailbox tools (do not shell out to CLI):

Team mailbox tools:

1. Pull inbox:
   `actor_inbox` with `{{"run_id":"<run-id>","limit": 20}}`
2. Acknowledge a message after processing:
   `actor_ack` with `{{"run_id":"<run-id>","message_id": 123}}`
3. Send a local message:
   `actor_send` with `{{"run_id":"<run-id>","to_actor_id":"worker","payload":{{"text":"..."}}}}`
4. Send a remote message:
   `actor_send` with `{{"run_id":"<run-id>","to_actor_id":"remote-worker","transport":"remote","route":{{"endpoint":"https://..."}},"payload":{{"text":"..."}}}}`
5. Force duplicate delivery when business logic requires repeated send:
   `actor_send` with `{{"run_id":"<run-id>","to_actor_id":"worker","allow_duplicate":true,"payload":{{"text":"..."}}}}`
6. Use explicit idempotency key when coordinating retries across workers:
   `actor_send` with `{{"run_id":"<run-id>","to_actor_id":"worker","idempotency_key":"stable-key","payload":{{"text":"..."}}}}`

Team context tool:

7. Inspect live team runtime status, roster, identity-card descriptions, and optional run step overlay:
   `team_members` with `{{}}`
8. When you need step-level overlay for a specific run:
   `team_members` with `{{"run_id":"<run-id>"}}`

Protocol rules:

- Always pull inbox before starting a new coordination step.
- In each turn, the first mailbox action must be `actor_inbox` before planning/coding.
- Before routing work based on teammate assumptions, inspect `team_members`.
- Treat `team_members` as the single Team context snapshot tool: it returns runtime summary, roster/card data, and optional run overlay.
- Treat `current_run_id` as a convenience default only; pass `run_id` explicitly whenever you are operating on a different run.
- If inbox has pending items, process and `actor_ack` them before emitting final result.
- Acknowledge each consumed message exactly once.
- Keep payload JSON compact and deterministic.
- Use `channel` only when a non-default channel is required.
- By default, `actor_send` auto-generates an idempotency key from message fields to prevent duplicate delivery on retries.
- Reuse the same payload and routing fields when retrying; changing payload under the same idempotency key will be rejected.
- Use `allow_duplicate=true` only when you intentionally need repeated delivery of equivalent payloads.
"#,
        team_section = team_section,
        current_run_section = current_run_section,
        actor_id = context.actor_id,
        default_channel = context.default_channel,
        continuity_section = continuity_section,
    );
    build_skill(
        "agenthub-actor-runtime".to_string(),
        format!("builtin://agenthub/actor-runtime/{}", context.actor_id),
        &instructions,
    )
}

fn build_continuity_section(context: &AcpActorSkillContext) -> String {
    let Some(continuity) = context.continuity.as_ref() else {
        return String::new();
    };
    let summary = truncate_chars(continuity.summary_text.as_str(), 400);
    let history_window = truncate_chars(continuity.history_window.to_string().as_str(), 800);
    let source_session = continuity
        .source_session_id
        .as_deref()
        .unwrap_or("n/a")
        .to_string();
    format!(
        r#"
- `continuity_mode`: `{mode}`
- `continuity_source_run_id`: `{source_run_id}`
- `continuity_source_session_id`: `{source_session_id}`
- `continuity_summary`: `{summary}`
- `continuity_history_window_json`: `{history_window}`
"#,
        mode = continuity.mode,
        source_run_id = continuity.source_run_id,
        source_session_id = source_session,
        summary = summary,
        history_window = history_window,
    )
}

#[cfg(test)]
mod tests {
    use super::{AcpActorSkillContext, build_actor_runtime_skill};

    #[test]
    fn actor_runtime_skill_includes_context_and_native_tool_contract() {
        let skill = build_actor_runtime_skill(&AcpActorSkillContext {
            team_id: Some("team-7".to_string()),
            current_run_id: Some("run-42".to_string()),
            actor_id: "planner".to_string(),
            default_channel: "coordination".to_string(),
            actor_cli_path: "/tmp/agenthub".to_string(),
            member_role: Some("leader".to_string()),
            member_skills: Vec::new(),
            contract_version: None,
            continuity: None,
        });
        assert_eq!(skill.name, "agenthub-actor-runtime");
        assert_eq!(skill.path, "builtin://agenthub/actor-runtime/planner");
        assert!(skill.instructions.contains("team_id`: `team-7`"));
        assert!(skill.instructions.contains("current_run_id`: `run-42`"));
        assert!(skill.instructions.contains("actor_id`: `planner`"));
        assert!(
            skill
                .instructions
                .contains("default_channel`: `coordination`")
        );
        assert!(skill.instructions.contains("actor_inbox"));
        assert!(skill.instructions.contains("actor_ack"));
        assert!(skill.instructions.contains("actor_send"));
        assert!(skill.instructions.contains("team_members"));
        assert!(
            skill
                .instructions
                .contains("single Team context snapshot tool")
        );
        assert!(skill.instructions.contains("\"run_id\":\"<run-id>\""));
        assert!(!skill.instructions.contains("$AGENTHUB_ACTOR_CLI"));
    }
}
