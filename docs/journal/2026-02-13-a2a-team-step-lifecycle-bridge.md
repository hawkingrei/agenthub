# A2A Team Step Lifecycle Bridge

## Summary

Expose scheduler-facing Team Step lifecycle APIs and service wiring so the
orchestrator can drive persisted step transitions without direct database
access.

## Background

Team Phase 1 already persisted step transitions (`submitted` -> `working` ->
terminal states) and converged run status. The missing piece was an explicit
bridge for scheduler workers to submit and transition steps through stable API
contracts.

## Scope

- `src/team/manager.rs`
- `src/api/teams.rs`
- `docs/todo.md`

## Key Decisions

- Keep TeamManager as the source of truth for lifecycle transitions and run
  status convergence; API handlers only validate boundary inputs and route calls.
- Add run-scoped step APIs:
  - `GET /api/teams/runs/:run_id/steps`
  - `POST /api/teams/runs/:run_id/steps`
  - `POST /api/teams/runs/:run_id/steps/:step_id/start`
  - `POST /api/teams/runs/:run_id/steps/:step_id/complete`
  - `POST /api/teams/runs/:run_id/steps/:step_id/fail`
- Enforce run/step ownership checks:
  - return `404` when run is missing,
  - return `404` when step is missing or does not belong to the run.
- Keep deterministic HTTP semantics for scheduler retries:
  - duplicate `(run_id, step_key, attempt)` submit returns `409`,
  - invalid step payloads (empty `step_key`, `member_id`, `error_text`,
    duplicate/empty `depends_on`) return `400`.
- Add `TeamManager::list_steps(run_id)` for service-level step enumeration.

## Validation

```bash
cargo test team_run_steps_api_supports_scheduler_lifecycle_bridge -- --nocapture
cargo test teams_router_http_contract -- --nocapture
cargo test team::manager::tests::list_steps_returns_sorted_steps_for_a_run -- --nocapture
```

## Follow-ups

- Add explicit `input_required` and `resume` scheduler APIs for human-in-the-loop
  coordination.
- Orchestrator worker now dispatches ready submitted steps in backend service
  loop (see `docs/journal/2026-02-15-a2a-team-orchestrator-worker-agent-start.md`).
- Validate worker behavior against real remote executors and end-to-end retries.
