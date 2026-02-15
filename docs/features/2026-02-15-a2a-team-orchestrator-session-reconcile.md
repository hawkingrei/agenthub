# A2A Team Orchestrator Working Step Session Reconcile

## Summary

Add orchestrator-side reconciliation from `team_steps.remote_task_id` to
`agent_sessions.status` so single-node Team runs can converge automatically
without manual step lifecycle API calls.

## Background

The worker already bootstraps team DAG steps and starts member agents, but
`working` steps were not automatically driven to terminal states after the
member process exited. In single-node mode, this caused runs to stall at
`working` unless a separate caller explicitly called `complete_step` or
`fail_step`.

## Scope

- `src/team/manager.rs`
- `src/team/orchestrator.rs`
- `docs/todo.md`

## Key Decisions

1. Add `TeamManager::get_agent_session_status(session_id)` as a narrow query
   helper for orchestrator reconciliation.
2. In each orchestrator tick, reconcile existing `working` steps before
   dispatching new submitted steps:
   - session status `completed` => `complete_step(step_id, None)`
   - session status `failed` / `cancelled` / `exited` => `fail_step(step_id, ...)`
3. Keep reconciliation idempotent by reusing existing step transition guards in
   `TeamManager` (`WHERE status IN (...)`) and tolerate repeated ticks.
4. Extend orchestrator test coverage with session-driven convergence cases:
   - `working -> completed` updates both step and run terminal status
   - `working -> failed` updates both step and run terminal status

## Validation

```bash
cargo test team::orchestrator::tests -- --nocapture
```

## Follow-ups

- Attach structured step outputs (instead of `None`) when member session
  terminal data becomes queryable from local ACP/event persistence.
- Add API/integration coverage that verifies full run convergence through
  Team HTTP endpoints in addition to manager-level assertions.
