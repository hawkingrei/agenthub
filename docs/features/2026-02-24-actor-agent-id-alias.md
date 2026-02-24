# Actor Runtime CLI/MCP Agent ID Alias

## Background

Actor mailbox runtime keeps `run_id` as the mailbox partition key, while user/operator mental model
often uses `agent_id`.

Before this change, runtime tools only accepted `actor_id` naming in CLI/MCP entrypoint flags, which
made operations less aligned with Team UI terminology.

## Scope

- Keep `run_id` semantics unchanged (required mailbox namespace key).
- Add `agent_id` aliases in actor runtime command parsing:
  - `agenthub actor inbox`: `--agent-id` alias for `--actor-id`
  - `agenthub actor ack`: `--agent-id` alias for `--actor-id`
  - `agenthub actor send`:
    - `--from-agent-id` alias for `--from-actor-id`
    - `--to-agent-id` alias for `--to-actor-id`
  - `agenthub actor-mcp`: `--agent-id` alias for `--actor-id`
- Add environment fallback alias:
  - `AGENTHUB_ACTOR_AGENT_ID` (alongside existing `AGENTHUB_ACTOR_ID`)
- Add explicit human/agent identity markers on mailbox message records:
  - `from_actor_kind`: `agent | human`
  - `to_actor_kind`: `agent | human`
  - inferred server-side from actor id (`user` / `human` aliases -> `human`)

## Key Decisions

1. Preserve `run_id` as canonical partition key.

- Avoid cross-run message mixing for inbox/ack/idempotency.
- Keep continuity/event replay/index model unchanged.

2. Expose `agent_id` aliases at tool boundary only.

- Improve operator UX and naming consistency.
- Keep DB schema unchanged; identity kind is computed at read-time and returned in API payloads.

## Files

- `src/actor_cli.rs`
- `src/actor_mcp.rs`
- `crates/agenthub-team-actor/src/message.rs`
- `src/team/manager/codec.rs`
- `src/team/manager/mailbox.rs`
- `src/api/openapi/spec.rs`
- `web/src/api.ts`
- `docs/todo.md`

## Validation

Suggested checks:

```bash
cargo test parse_inbox_accepts_agent_id_alias_flag
cargo test parse_inbox_uses_agent_id_env_fallback
cargo test parse_send_accepts_agent_id_alias_flags
cargo test parse_actor_mcp_context_uses_agent_id_env_alias
cargo test parse_actor_mcp_context_accepts_agent_id_flag
cargo test -p agenthub-team-actor
cargo test openapi -- --nocapture
```
