# Summary

Rolled back the run-scoped eager member-session model after clarifying the intended Team lifecycle:

- leader/worker sessions are persistent member runtimes;
- `run` is task/step execution state only;
- `create_team` should eventually start persistent member sessions, but that requires a protocol refactor before implementation.

## Why

The previous eager-startup patch introduced `team_run_member_sessions` and treated member sessions as run-owned resources. That conflicted with the intended model:

- a Team member (`leader`, `worker-*`) is a long-lived actor identity;
- a Team run is only the execution state for one task context and its step graph;
- a run should not own or pre-bind member sessions.

Further inspection showed the deeper issue is protocol-level:

- actor runtime skill embeds a fixed `run_id`;
- `actor-mcp` requires `--run-id`;
- actor CLI mailbox commands are run-scoped by construction.

Because of that, "persistent member sessions" cannot be implemented safely by only changing orchestrator behavior.

## What Changed

- Removed the run-scoped `team_run_member_sessions` binding model from active code paths.
- Removed orchestrator eager member startup and prestarted-session reuse.
- Restored orchestrator behavior so step dispatch starts/uses the member agent directly for the active step.
- Kept the `team_members` runtime query, but changed its top-level `session_id/session_status` view to read the member agent's current live running session instead of a run-owned binding.
- Updated Team run snapshot member session status to use the member agent's current live running session instead of a run-owned binding.
- Restored hidden-team-agent filtering to only hide agents that are actively attached to working/input-required Team steps.

## Current Model

After this rollback:

- `team_members(run_id)` still provides the run overlay for step status;
- top-level member session visibility reflects the member agent's current live runtime session;
- run state and session ownership are no longer artificially coupled by `team_run_member_sessions`.

## Status After Follow-Up

The protocol refactor was completed far enough to land the first Team lifecycle controls:

- `create_team` now eagerly starts member runtimes;
- `POST /api/teams/:id/start` and `POST /api/teams/:id/stop` are available;
- orchestrator dispatch now requires an already-running member runtime and no longer owns session startup.

This is enough to align ownership with the intended model:

- Team members own runtime sessions;
- runs own task/step execution state only.

What is **not** finished is a richer Team-owned runtime registry and UI/runtime reconciliation layer.

## Next Step

The remaining work is now narrower:

1. add a clearer Team-owned runtime state model for UI/CLI inspection and reconciliation;
2. wire `/teams` and CLI controls onto the new `create/start/stop` lifecycle;
3. decide whether `remote_task_id` should be renamed to a more neutral runtime-handle field.

## Validation

Suggested validation after the rollback:

- `cargo test describe_run_members_returns_live_roster_and_session_state -- --nocapture`
- `cargo test team::orchestrator::tests -- --nocapture`
- `cargo test actor_mcp -- --nocapture`
- `cargo test team_run_snapshot_api_returns_member_status_and_mailbox_summary -- --nocapture`
- `cargo test teams_api_create_team_auto_starts_member_runtime -- --nocapture`
- `cargo test teams_api_start_and_stop_team_runtime -- --nocapture`
