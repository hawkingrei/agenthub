# A2A Team Run Status Convergence

## Summary

Implement deterministic run status convergence in Team Manager so run status
progresses from `working` to terminal states based on persisted step outcomes.

## Background

Team step lifecycle persistence was already available (`submitted`, `working`,
terminal step states), but run status remained `working` after terminal step
events. Scheduler integration needs run-level terminal state and events to make
polling and replay logic deterministic.

## Scope

- `src/team/mod.rs`
- `src/team/manager.rs`
- `docs/todo.md`

## Key Decisions

- On `complete_step`:
  - emit `step_completed` as before,
  - evaluate all steps in the run; if every step is `completed`, transition
    run status to `completed`,
  - set run `ended_at` and emit `run_completed` exactly once.
- On `fail_step`:
  - emit `step_failed` as before,
  - transition run status to `failed` (if not already terminal),
  - set run `ended_at` and emit `run_failed` exactly once.
- Keep transitions idempotent via SQL status guards:
  - run updates only apply from non-terminal states
    (`submitted`/`working`/`input_required`).
- Extend manager tests to verify:
  - run terminal status + `ended_at` for both complete/fail paths,
  - multi-step behavior where run stays `working` until all steps complete.

## Validation

```bash
cargo test team::manager -- --nocapture
```

## Follow-ups

- Add scheduler-facing API/service bridge that drives step transitions from the
  orchestrator worker loop.
- Add input-required and resume transitions for step/run coordination.
