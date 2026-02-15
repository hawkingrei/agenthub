# A2A Team Orchestrator Worker Member Start

## Summary

Add a backend orchestrator worker loop that scans ready Team steps and starts
member agents with per-step actor runtime context.

## Background

Team APIs already persisted run/step lifecycle, but no background scheduler was
driving submitted steps into execution. We already removed env-based actor
runtime discovery and introduced explicit actor context on agent start, so the
next step is wiring a worker that actually uses it.

## Scope

- `src/team/orchestrator.rs`
- `src/team/mod.rs`
- `src/team/manager.rs`
- `src/team/manager/tests.rs`
- `src/state.rs`
- `docs/todo.md`

## Key Decisions

1. Add `TeamOrchestratorWorker` with periodic tick:
   - scan active runs (`submitted` / `working` / `input_required`),
   - bootstrap run steps from team spec when no persisted steps exist,
   - evaluate submitted steps,
   - dispatch only when every dependency step is `completed`.
2. Dispatch strategy for a ready step:
   - start member agent via `AgentManager::start_agent_with_actor_context(...)`,
   - pass actor context using `run_id` + `member_id` (`actor_id`) and default
     `channel=default`,
   - persist lifecycle transition with `start_step(step_id, remote_task_id=session_id)`.
3. On member start failure, mark the step failed to avoid silent starvation.
4. Keep worker settings explicit (`poll_interval_secs`,
   `max_dispatch_per_tick`) and start it from `AppState::init`.
5. Add `TeamManager::list_active_runs(limit)` as the worker-facing query helper.
6. Introduce a `TeamMemberAgentStarter` port so worker dispatch is testable
   without spawning real agent subprocesses.
7. Add worker-level integration tests to verify:
   - injected actor runtime context (`run_id` / `actor_id`) at dispatch time,
- mailbox `send` -> `inbox` -> `ack` flow against the injected actor id,
- failed member startup marks step as `failed` deterministically.
- spec-step bootstrap dispatch respects dependency order (`depends_on`) across
  multiple worker ticks.

## Validation

```bash
cargo test team::orchestrator::tests -- --nocapture
cargo test list_active_runs_returns_non_terminal_runs_only -- --nocapture
```

## Follow-ups

- Connect worker dispatch with full orchestrator step submission lifecycle so
  ready steps are derived directly from team spec DAG execution plan.
- Add integration tests for worker-driven end-to-end actor inbox/ack flow.
