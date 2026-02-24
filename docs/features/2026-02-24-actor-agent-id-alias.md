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

## Key Decisions

1. Preserve `run_id` as canonical partition key.

- Avoid cross-run message mixing for inbox/ack/idempotency.
- Keep continuity/event replay/index model unchanged.

2. Expose `agent_id` aliases at tool boundary only.

- Improve operator UX and naming consistency.
- No schema/table/API contract changes required.

## Files

- `src/actor_cli.rs`
- `src/actor_mcp.rs`
- `docs/todo.md`

## Validation

Suggested checks:

```bash
cargo test parse_inbox_accepts_agent_id_alias_flag
cargo test parse_inbox_uses_agent_id_env_fallback
cargo test parse_send_accepts_agent_id_alias_flags
cargo test parse_actor_mcp_context_uses_agent_id_env_alias
cargo test parse_actor_mcp_context_accepts_agent_id_flag
```

