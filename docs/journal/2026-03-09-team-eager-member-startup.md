# Summary

Added run-scoped eager member session startup so leader and worker sessions come up when a Team run starts, while the step dependency graph still controls execution order.

## Why

`team_members` can now expose live roster and identity-card state, but the previous runtime model still only started the leader session when `leader_plan` became ready. Worker sessions stayed offline until their dependent steps were dispatched, which meant:

- leader could not see live worker session state at run start;
- worker inbox/session warm-up was delayed until after planning completed;
- `/api/teams/:id/runs/:run_id/snapshot` and hidden-agent filtering did not reflect the intended "all members are online" startup model.

The requirement is stricter than "workers should eventually run": leader and worker sessions should both start when the Team run starts, without removing the existing step dependency graph.

## What Changed

- Added a run-member binding table:
  - `team_run_member_sessions(run_id, member_id, session_id, created_at, updated_at)`
- Added `TeamManager` helpers to:
  - bind a session to a `(run_id, member_id)` pair;
  - query run-bound member sessions;
  - expose bound session ids/statuses in `describe_run_members(...)`.
- Updated Team run snapshot assembly so member status can fall back to the bound run-member session even before a step transitions to `working`.
- Updated hidden-team-agent filtering so eager-started team member sessions remain hidden from the normal agent list even before a step writes `remote_task_id`.
- Updated the orchestrator to:
  - eagerly start sessions for all members once the run has been bootstrapped and all steps are still `submitted`;
  - keep the step graph unchanged (`leader_plan` still dispatches first);
  - reuse an already-bound running member session when the step later becomes ready instead of spawning a duplicate session.
- Kept eager startup best-effort:
  - member-session startup failures are recorded as run events;
  - a failed eager start does not abort the whole tick;
  - step dispatch can still start the member later if no reusable running session exists.

## Validation

Suggested validation:

- `cargo test team::orchestrator::tests -- --nocapture`
- `cargo test describe_run_members_returns_live_roster_and_session_state -- --nocapture`
- `cargo test team_run_snapshot_api_returns_member_status_and_mailbox_summary -- --nocapture`
- `cargo test actor_mcp -- --nocapture`

Manual/runtime checks:

- create a Team run and confirm `team_members` shows leader + workers with session ids/statuses immediately after run bootstrap;
- confirm worker steps remain `submitted` until `leader_plan` completes;
- confirm the `/teams` run snapshot shows bound worker session state before worker step dispatch;
- confirm eager-started team member sessions stay hidden from the normal Agents page.

## Known Limitation

This change does not solve concurrent active Team runs reusing the same member agent. The current runtime still effectively assumes one active member session per member identity. The new binding table makes run-local startup and visibility correct, but multi-run concurrency still needs a separate design.
