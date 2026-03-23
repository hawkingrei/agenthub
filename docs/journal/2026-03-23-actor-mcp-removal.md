# Actor MCP Removal From Mainline

## Summary

- Removed the legacy `actor-mcp` entrypoint from the main AgentHub binary path.
- Kept `agenthub actor ...` as the only active actor coordination entrypoint on
  mainline.
- Updated active Team prompts, tests, and feature docs so they no longer point
  at actor MCP review/tool flows.

## Why

- Team runtime is now CLI-first through `AGENTHUB_ACTOR_CLI`.
- Keeping `actor-mcp` on the main path preserved duplicate coordination
  contracts and stale prompt guidance.
- The cleanup reduces prompt/runtime drift and removes an obsolete control path
  from active documentation and fixtures.

## What Changed

- Removed the `actor_mcp` module wiring from `src/lib.rs` and `src/app.rs`.
- Deleted `src/actor_mcp.rs`.
- Updated Team test fixtures to launch `agenthub actor` instead of
  `actor-mcp`.
- Removed agent-facing prompt references to
  `acp_permission_review_respond`; permission review is now described as
  runtime-controlled review flow.
- Updated active feature docs and TODO references so `actor-foundation.md`
  is the canonical CLI-first runtime reference and
  `team-mcp-enforcement.md` is explicitly historical background.

## Validation

- `cargo test -p agenthub actor_cli::tests -- --nocapture`
- `cargo test -p agenthub api::teams::tests -- --nocapture`
- `cd web && npx vitest run src/pages/team/member_helpers.test.ts src/pages/team/create_helpers.test.ts`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`
- `cargo fmt --all --check`
- `git -c core.fsmonitor=false diff --check`
