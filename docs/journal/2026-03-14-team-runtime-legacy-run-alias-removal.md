# Summary

Removed the legacy `AGENTHUB_ACTOR_RUN_ID` environment alias from Team actor-runtime startup and mailbox tooling.

## Why

The Team member-runtime rollout already moved the runtime model to:

- `team_id` as the stable Team scope
- `current_run_id` as an optional execution overlay

Keeping `AGENTHUB_ACTOR_RUN_ID` as an extra environment alias preserved a fixed-run mental model and extended compatibility debt across:

- agent process startup
- `agenthub actor`
- `agenthub actor-mcp`
- Team runtime tests

At this point the remaining callers can use either:

- explicit `run_id` tool/CLI arguments, or
- `AGENTHUB_ACTOR_CURRENT_RUN_ID`

so the legacy alias was no longer necessary.

## What Changed

- Stopped exporting `AGENTHUB_ACTOR_RUN_ID` when starting actor-scoped agent processes.
- Stopped adding `AGENTHUB_ACTOR_RUN_ID` to ACP mailbox MCP server environment.
- Removed CLI and MCP fallback parsing from `AGENTHUB_ACTOR_RUN_ID`.
- Updated Team runtime env tests to assert only:
  - `AGENTHUB_ACTOR_TEAM_ID`
  - `AGENTHUB_ACTOR_CURRENT_RUN_ID`
  - `AGENTHUB_ACTOR_ID`
  - `AGENTHUB_ACTOR_CHANNEL`
  - `AGENTHUB_ACTOR_CLI`
- Added regression coverage to ensure legacy env alias no longer drives run resolution.

## Validation

Suggested validation commands:

- `cargo test -p agenthub parse_inbox_ignores_legacy_run_env_alias -- --nocapture`
- `cargo test -p agenthub parse_actor_mcp_context_ignores_legacy_run_env_alias -- --nocapture`
- `cargo test -p agenthub start_agent_with_actor_context_injects_runtime_env_vars -- --nocapture`
- `cargo test -p agenthub start_route_rejects_actor_runtime_payload_for_agent_mode -- --nocapture`
- `cargo test -p agenthub-acp build_actor_mailbox_mcp_server_uses_actor_runtime_binary_and_context -- --nocapture`
