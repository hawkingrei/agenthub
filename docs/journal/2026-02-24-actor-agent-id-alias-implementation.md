# 2026-02-24 Actor Agent-ID Alias Implementation

## Context

Team operator workflows discussed actor identity in `agent_id` terms, while runtime tooling exposed `actor_id` naming and strict `run_id` partitioning. We aligned tool ergonomics without changing mailbox partition contracts.

## Changes

1. Added `agent_id` aliases in actor runtime entrypoints:
   - `agenthub actor inbox`: `--agent-id`
   - `agenthub actor ack`: `--agent-id`
   - `agenthub actor send`: `--from-agent-id`, `--to-agent-id`
   - `agenthub actor-mcp`: `--agent-id`

2. Added environment fallback alias:
   - `AGENTHUB_ACTOR_AGENT_ID`

3. Added explicit mailbox identity markers:
   - `from_actor_kind: agent | human`
   - `to_actor_kind: agent | human`
   - kind inferred server-side from actor identity conventions

4. Updated OpenAPI/web types to expose the new identity-kind fields.

5. Updated docs/todo verification item to reference canonical feature contract.

## Files Touched

- `src/actor_cli.rs`
- `src/actor_mcp.rs`
- `crates/agenthub-team-actor/src/message.rs`
- `crates/agenthub-team-actor/src/lib.rs`
- `crates/agenthub-team-actor/src/mailbox_tests.rs`
- `src/team/manager/codec.rs`
- `src/team/manager/mailbox.rs`
- `src/team/manager/tests.rs`
- `src/api/openapi/spec.rs`
- `web/src/api.ts`
- `docs/journal/2026-02-24-actor-agent-id-alias.md`
- `docs/todo.md`

## Validation Notes

Executed during implementation:

```bash
cargo test -p agenthub-team-actor
cargo test actor_messages_support_inbox_and_ack_flow -- --nocapture
cargo test remote_actor_messages_relay_success_marks_message_delivered -- --nocapture
cargo test parse_inbox_accepts_agent_id_alias_flag -- --nocapture
cargo test parse_actor_mcp_context_accepts_agent_id_flag -- --nocapture
cargo test openapi -- --nocapture
pnpm -C web run build
```

## Result

Runtime now supports agent-oriented naming at tool boundaries while preserving run-scoped mailbox isolation and replay semantics.
