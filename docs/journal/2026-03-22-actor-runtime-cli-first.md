# Actor Runtime CLI-First Migration

## Summary

- Moved Team actor runtime guidance from mailbox MCP tool calls to
  `AGENTHUB_ACTOR_CLI` commands.
- Added CLI coverage for Team runtime operations that were still described as
  MCP-only:
  - `team-tasks`
  - `team-task-create`
  - `team-task-update`
  - `time-trigger-set`
  - `time-trigger-list`
  - `time-trigger-cancel`
  - `permission-review-respond`
- Added shared actor runtime env helpers so CLI and the legacy compatibility
  entrypoint reuse the same internal gRPC mailbox configuration parsing.
- Stopped ACP runtime auto-injection of the `agenthub-actor-mailbox` MCP server.
  Team actor sessions now rely on skills + `AGENTHUB_ACTOR_CLI` for mailbox/task
  coordination.

## Files

- `src/actor_cli.rs`
- `src/actor_runtime_env.rs`
- `src/actor_mcp.rs`
- `crates/agenthub-acp/src/actor_runtime_skill.rs`
- `crates/agenthub-acp/src/lib.rs`
- `crates/agenthub-team-prompts/src/lib.rs`
- `skills/team/*.md`
- `docs/features/*.md`
- `docs/todo.md`

## Validation

- `cargo check --workspace --locked`
- `cargo test -p agenthub actor_cli::tests -- --nocapture`
- `cargo test -p agenthub-acp actor_runtime_skill -- --nocapture`
- `cargo clippy --workspace --all-targets -- -D warnings`
