# Summary

Completed phase 1 of the Team member-runtime refactor: actor session context no longer treats
`run_id` as the only fixed runtime anchor.

The current session context now carries:

- `team_id` as the stable Team scope
- `current_run_id` as an optional convenience overlay

Mailbox operations still execute against a concrete `run_id`, but that `run_id` is now resolved
per tool/CLI operation instead of being required as the immutable session identity.

## Why

The intended Team model is:

- Team members (`leader`, `worker-*`) own persistent runtime sessions
- Team runs own only task/step execution state

The previous protocol violated that model because actor runtime, actor MCP, and actor CLI all
assumed a fixed session-scoped `run_id`.

That made it impossible to later implement:

- `create_team` starting persistent member runtimes
- explicit `start_team` / `stop_team`
- run dispatch onto already-running member sessions

without context leakage across runs.

## What Changed

### Actor runtime context

- `AcpActorSkillContext` now stores:
  - `team_id: Option<String>`
  - `current_run_id: Option<String>`
- legacy `AGENTHUB_ACTOR_RUN_ID` remains injected as a compatibility alias when
  `current_run_id` exists

### Actor MCP

- `actor-mcp` now accepts optional `--team-id` and optional `--run-id`
- MCP tool calls resolve `run_id` in this order:
  1. explicit tool argument
  2. `current_run_id` from session context
  3. error
- `actor_inbox`, `actor_ack`, `actor_send`, and `team_members` all follow that rule

### Actor CLI

- actor CLI mailbox commands now resolve `run_id` from:
  1. explicit `--run-id`
  2. `AGENTHUB_ACTOR_CURRENT_RUN_ID`
  3. legacy `AGENTHUB_ACTOR_RUN_ID`

### Team orchestrator

- member actor runtime injection now carries:
  - `team_id = run.team_id`
  - `current_run_id = run.id`

### API parsing

- `/api/agents` actor-runtime parsing now accepts optional `team_id` and optional `run_id`
- at least one of them must be present

## Current Behavior

After phase 1:

- Team actor sessions are no longer semantically forced to be run-owned
- mailbox operations still require a concrete run overlay, but it is operation-scoped
- compatibility env injection is preserved so existing runtime consumers do not break immediately

This is intentionally a transitional state.

## Status After Follow-Up

Phase 1 enabled the first lifecycle integration:

1. `create_team` now starts leader + workers immediately
2. explicit `start_team` / `stop_team` API endpoints exist
3. orchestrator dispatches work onto existing member sessions instead of starting sessions itself

The remaining compatibility debt is narrower:

1. richer Team-owned runtime reconciliation/state exposure
2. UI/CLI wiring around manual `start_team` / `stop_team`
3. eventual removal of the legacy fixed-run compatibility alias once downstream consumers are migrated

## Validation

Suggested validation commands for phase 1:

- `cargo test actor_mcp -- --nocapture`
- `cargo test parse_start_actor_runtime_context -- --nocapture`
- `cargo test team::orchestrator::tests -- --nocapture`
- `cargo test start_agent_with_actor_context_injects_runtime_env_vars -- --nocapture`
- `cargo test describe_run_members_returns_live_roster_and_session_state -- --nocapture`
- `cargo test teams_api_create_team_auto_starts_member_runtime -- --nocapture`
- `cargo test teams_api_start_and_stop_team_runtime -- --nocapture`
