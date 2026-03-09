## Summary

Added a runtime team-roster query path so leader and worker sessions can inspect the live team membership and identity-card view during execution instead of relying only on static prompt text.

## Why

The existing Team prompt mentions `spec.members[].description` and `.well-known/agent-card`, but runtime actor sessions had no MCP/CLI path to inspect the current roster, step status, or member session liveness. That breaks down once membership becomes dynamic.

## What Changed

- Added `TeamManager::describe_run_members(run_id)` to aggregate:
  - team identity
  - member display names
  - static role/description
  - synthesized identity-card view
  - current step status
  - current session id / session status
- Added actor MCP tool `team_members`.
- Added CLI command:
  - `agenthub actor team-members --run-id <run_id>`
- Updated actor runtime skill instructions to require checking `team_members` before making teammate-routing assumptions.
- Updated default team leader/worker prompts so `spec.members[]` is treated as static baseline and `team_members` is treated as the live execution view.

## Validation

Local targeted validation covered:

- `cargo test actor_mcp -- --nocapture`
- `cargo test describe_run_members -- --nocapture`
- `cargo test parse_team_members -- --nocapture`
- `cargo test -p agenthub-acp actor_runtime_skill_includes_context_and_native_tool_contract -- --nocapture`
- `cargo test -p agenthub-team-prompts prompt_templates_keep_required_contract_lines -- --nocapture`

## Follow-up

- Team run startup still dispatches only ready steps. Leader/worker eager session startup should be handled separately so all member sessions come up at run start without removing the step dependency graph.
- `team_steps.remote_task_id` still stores the member session id. The name is now misleading and should be normalized in a later compatibility-safe refactor.
