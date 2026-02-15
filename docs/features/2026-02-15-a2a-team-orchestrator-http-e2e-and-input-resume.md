# A2A Team Orchestrator HTTP E2E and Input Resume Retry Coverage

## Summary

Expand Team orchestrator validation with:

- router-level HTTP end-to-end convergence test using a real local executor,
- orchestrator reconciliation coverage for `input_required` / `resume` with idempotent retries.

## Background

We already had orchestrator dispatch, DAG bootstrap, and `working` session
reconciliation. Two follow-up gaps remained:

1. verify lifecycle bridge behavior through Team HTTP APIs with a real member
   process,
2. verify orchestrator behavior around `input_required` and `resume` transitions
   under retry/idempotent semantics.

## Scope

- `src/team/orchestrator.rs`
- `src/api/teams/tests.rs`
- `src/api/teams/tests_router.rs`
- `docs/todo.md`

## Key Decisions

1. Orchestrator session reconciliation now also tracks `input_required` steps
   (not just `working`) when a `remote_task_id` is present.
2. Session status mapping keeps existing terminal semantics:
   - `completed` => `complete_step`
   - `failed` / `cancelled` / `exited` => `fail_step`
3. Add orchestration test cases for:
   - `input_required` -> `resume` flow with repeated retry calls that should be
     idempotent,
   - `input_required` step directly reconciled to failed from terminal session.
4. Add a router-level end-to-end test that:
   - creates a real local member agent (`/bin/sh -lc ...`),
   - creates Team + run through HTTP routes,
   - drives worker ticks and polls run/steps/events via HTTP until completion.

## Validation

```bash
cargo test team::orchestrator::tests -- --nocapture
cargo test teams_router_orchestrator_converges_with_real_executor -- --nocapture
```

## Follow-ups

- Add similar HTTP E2E for a multi-step DAG with parallel branches and strict
  event ordering assertions.
- Add failure-path E2E with non-zero member process exit plus retry visibility
  checks in run events.
