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

Use the actor CLI for runtime coordination:

Team mailbox commands:

1. Pull inbox:
   `agenthub actor inbox --run-id "<run-id>" --limit 20`
2. Acknowledge a message after processing:
   `agenthub actor ack --run-id "<run-id>" --message-id 123`
3. Send a local direct message:
   `agenthub actor send --run-id "<run-id>" --to-actor-id "worker" --text "Please review this patch.\n\n- verify API shape\n- call out blockers"`
4. Send a channel message:
   `agenthub actor send --run-id "<run-id>" --channel-id "all" --text "@worker Please review this patch.\n\n- verify API shape\n- call out blockers"`
5. Send a remote direct message:
   `agenthub actor send --run-id "<run-id>" --to-actor-id "remote-worker" --transport remote --route-json '{{"endpoint":"https://..."}}' --text "Please review this patch.\n\n- verify API shape\n- call out blockers"`
6. Send an urgent human notification:
   `agenthub actor send --run-id "<run-id>" --to-actor-id "user" --text "Urgent: permission review timed out. Please check Channel for details."`
7. Force duplicate delivery when business logic requires repeated send:
   `agenthub actor send --run-id "<run-id>" --to-actor-id "worker" --allow-duplicate --text "Reminder:\n\n- update the test evidence\n- reply when done"`
8. Use explicit idempotency key when coordinating retries across workers:
   `agenthub actor send --run-id "<run-id>" --to-actor-id "worker" --idempotency-key "stable-key" --text "Reminder:\n\n- update the test evidence\n- reply when done"`

Team context commands:

9. Inspect live team runtime status, roster, identity-card descriptions, and optional run step overlay:
   `agenthub actor team-members`
10. When you need step-level overlay for a specific run:
   `agenthub actor team-members --run-id "<run-id>"`

Protocol rules:

- Always pull inbox before starting a new coordination step.
- In each turn, the first mailbox action must be `actor inbox` before planning/coding.
- Treat `actor inbox` output as a live unread snapshot: it now includes `pending_count` alongside the fetched messages.
- Mailbox nudges are token-efficient by default: only direct `agent -> agent` sends and leader-authored channel `@member_id` mentions trigger immediate ACP hints.
- Other unread mailbox traffic may surface later as one compact unread summary after roughly 3 minutes of ACP output silence; if unread count is `0`, no reminder is sent.
- Before routing work based on teammate assumptions, inspect `actor team-members`.
- Treat `actor team-members` as the single Team context snapshot command: it returns runtime summary, roster/card data, per-member `pending_inbox_count`, and optional run overlay.
- Treat `current_run_id` as a convenience default only; pass `run_id` explicitly whenever you are operating on a different run.
- If inbox has pending items, process and `actor ack` them before emitting final result.
- Acknowledge each consumed message exactly once.
- Keep payload JSON compact and deterministic.
- Prefer `actor send --text` for markdown-rich messages; it preserves formatting better than wrapping prose inside structured fields.
- For group chat / channel sends, use `channel_id`; the message will still fan out to all relevant teammates even when `@member_id` appears in the text.
- Treat `@member_id` in channel text as mention metadata for receivers, not as a routing override.
- Use `to_actor_id = "user"` or `user:<id>` only when you intentionally want a human notification.
- Use `channel` only when a non-default channel is required.
- By default, `actor send` auto-generates an idempotency key from message fields to prevent duplicate delivery on retries.
- Reuse the same payload and routing fields when retrying; changing payload under the same idempotency key will be rejected.
- Use `allow_duplicate=true` only when you intentionally need repeated delivery of equivalent payloads.
- Use `payload` only when the receiver genuinely needs machine-readable fields such as `status`, `evidence`, or workflow metadata.
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
    fn actor_runtime_skill_includes_context_and_cli_contract() {
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
        assert!(skill.instructions.contains("agenthub actor inbox"));
        assert!(skill.instructions.contains("actor inbox"));
        assert!(skill.instructions.contains("actor ack"));
        assert!(skill.instructions.contains("actor send"));
        assert!(skill.instructions.contains("--channel-id \"all\""));
        assert!(skill.instructions.contains("--to-actor-id \"user\""));
        assert!(skill.instructions.contains("actor team-members"));
        assert!(
            skill
                .instructions
                .contains("single Team context snapshot command")
        );
        assert!(skill.instructions.contains("--run-id \"<run-id>\""));
    }
}
