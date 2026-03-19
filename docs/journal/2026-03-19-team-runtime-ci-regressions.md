## Summary

Aligned remaining `fix/team-runtime-bugfixes` CI regressions after the Team ACP default-view and trigger-tool changes.

## Changes

- Refactored `actor_mcp` JSON-RPC tool handling to pass a shared tool context instead of exceeding Clippy's argument limit.
- Updated the actor MCP tool-list test to include the new `agent_time_trigger_*` tools.
- Removed Clippy-reported needless borrows in actor MCP tests and Team mailbox reply persistence.
- Updated Team Playwright E2E flows to open the main `Runs` action through the scoped helper so the test no longer collides with duplicated `Runs` buttons.

## Validation

- `cargo test actor_mcp::tests::jsonrpc_tools_list_and_call_drive_local_mailbox_flow`
- `cargo test actor_mcp::tests::jsonrpc_team_members_returns_live_roster_view`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cd web && npm run lint -- tests/e2e/team_page.e2e.ts`
- `cargo fmt --all --check`
- `git -c core.fsmonitor=false diff --check`
