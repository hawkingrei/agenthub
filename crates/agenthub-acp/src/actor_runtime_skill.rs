use agenthub_acp_core::{AcpSkill, build_skill};

use crate::AcpActorSkillContext;

pub(super) fn build_actor_runtime_skill(context: &AcpActorSkillContext) -> AcpSkill {
    let instructions = format!(
        r#"# AgentHub Actor Runtime Skill

You are running inside an AgentHub actor session.

- `run_id`: `{run_id}`
- `actor_id`: `{actor_id}`
- `default_channel`: `{default_channel}`

Use the execute/terminal tool to interact with actor mailbox via CLI:

1. Pull inbox:
   `"$AGENTHUB_ACTOR_CLI" actor inbox --limit 20`
2. Acknowledge a message after processing:
   `"$AGENTHUB_ACTOR_CLI" actor ack --message-id <message_id>`
3. Send a local message:
   `"$AGENTHUB_ACTOR_CLI" actor send --to-actor-id <actor_id> --payload-json '{{"text":"..."}}'`
4. Send a remote message:
   `"$AGENTHUB_ACTOR_CLI" actor send --transport remote --to-actor-id <remote_actor_id> --route-json '{{"endpoint":"https://..."}}' --payload-json '{{"text":"..."}}'`
5. Force a duplicate send when business logic requires repeated delivery:
   `"$AGENTHUB_ACTOR_CLI" actor send --allow-duplicate --to-actor-id <actor_id> --payload-json '{{"text":"..."}}'`
6. Use explicit idempotency key when coordinating retries across tools/workers:
   `"$AGENTHUB_ACTOR_CLI" actor send --idempotency-key <stable_key> --to-actor-id <actor_id> --payload-json '{{"text":"..."}}'`

Protocol rules:

- Always pull inbox before starting a new coordination step.
- Acknowledge each consumed message exactly once.
- Keep payload JSON compact and deterministic.
- Use `--channel` only when a non-default channel is required.
- By default, `actor send` auto-generates an idempotency key from message fields to prevent duplicate delivery on retries.
- Reuse the same payload and routing fields when retrying; changing payload under the same idempotency key will be rejected.
- Use `--allow-duplicate` only when you intentionally need repeated delivery of equivalent payloads.
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
