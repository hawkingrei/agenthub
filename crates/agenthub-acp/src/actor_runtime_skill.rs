use agenthub_acp_core::{AcpSkill, build_skill};

use crate::AcpActorSkillContext;

pub(super) fn build_actor_runtime_skill(context: &AcpActorSkillContext) -> AcpSkill {
    let instructions = format!(
        r#"# AgentHub Actor Runtime Skill

You are running inside an AgentHub actor session.

- `run_id`: `{run_id}`
- `actor_id`: `{actor_id}`
- `default_channel`: `{default_channel}`

Use MCP native actor mailbox tools (do not shell out to CLI):

1. Pull inbox:
   `actor_inbox` with `{{"limit": 20}}`
2. Acknowledge a message after processing:
   `actor_ack` with `{{"message_id": 123}}`
3. Send a local message:
   `actor_send` with `{{"to_actor_id":"worker","payload":{{"text":"..."}}}}`
4. Send a remote message:
   `actor_send` with `{{"to_actor_id":"remote-worker","transport":"remote","route":{{"endpoint":"https://..."}},"payload":{{"text":"..."}}}}`
5. Force duplicate delivery when business logic requires repeated send:
   `actor_send` with `{{"to_actor_id":"worker","allow_duplicate":true,"payload":{{"text":"..."}}}}`
6. Use explicit idempotency key when coordinating retries across workers:
   `actor_send` with `{{"to_actor_id":"worker","idempotency_key":"stable-key","payload":{{"text":"..."}}}}`

Protocol rules:

- Always pull inbox before starting a new coordination step.
- Acknowledge each consumed message exactly once.
- Keep payload JSON compact and deterministic.
- Use `channel` only when a non-default channel is required.
- By default, `actor_send` auto-generates an idempotency key from message fields to prevent duplicate delivery on retries.
- Reuse the same payload and routing fields when retrying; changing payload under the same idempotency key will be rejected.
- Use `allow_duplicate=true` only when you intentionally need repeated delivery of equivalent payloads.
"#,
        run_id = context.run_id,
        actor_id = context.actor_id,
        default_channel = context.default_channel,
    );
    build_skill(
        "agenthub-actor-runtime".to_string(),
        format!("builtin://agenthub/actor-runtime/{}", context.actor_id),
        &instructions,
    )
}

#[cfg(test)]
mod tests {
    use super::{AcpActorSkillContext, build_actor_runtime_skill};

    #[test]
    fn actor_runtime_skill_includes_context_and_native_tool_contract() {
        let skill = build_actor_runtime_skill(&AcpActorSkillContext {
            run_id: "run-42".to_string(),
            actor_id: "planner".to_string(),
            default_channel: "coordination".to_string(),
            actor_cli_path: "/tmp/agenthub".to_string(),
        });
        assert_eq!(skill.name, "agenthub-actor-runtime");
        assert_eq!(skill.path, "builtin://agenthub/actor-runtime/planner");
        assert!(skill.instructions.contains("run_id`: `run-42`"));
        assert!(skill.instructions.contains("actor_id`: `planner`"));
        assert!(
            skill
                .instructions
                .contains("default_channel`: `coordination`")
        );
        assert!(skill.instructions.contains("actor_inbox"));
        assert!(skill.instructions.contains("actor_ack"));
        assert!(skill.instructions.contains("actor_send"));
        assert!(!skill.instructions.contains("$AGENTHUB_ACTOR_CLI"));
    }
}
