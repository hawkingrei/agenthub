# Team Attempt Projection

## Summary

- added a narrow backend projection for Team task execution attempts without changing database
  schema
- linked task status transitions now maintain `task.context.execution.attempt_number` as the
  canonical 1-based attempt counter

## Scope

- `src/team/manager.rs`
- `src/team/manager/tests.rs`

## What Changed

- centralized attempt tracking in `sync_linked_task_status_tx(...)` instead of scattering it across
  individual run/step lifecycle entry points
- increment `task.context.execution.attempt_number` only when a linked task enters
  `in_progress` from a non-`in_progress` state
- preserve the current attempt number when the task leaves active execution for `waiting`,
  `in_review`, `completed`, or `canceled`
- keep the same attempt number when a run rotates or restarts while the linked task is still part
  of the same active execution push

## Why

- `docs/features/team-execution-vocabulary.md` defines `attempt` as the semantic boundary for one
  bounded active execution try, but runtime state previously exposed only task status and run
  status
- adding an additive projection in task context lets API and UI surfaces adopt the new vocabulary
  incrementally without forcing an immediate schema migration

## Validation

- `cargo fmt --all --check`
- `cargo test -p agenthub linked_run_create_sets_first_attempt_number -- --nocapture`
- `cargo test -p agenthub linked_run_input_required_and_resume_sync_task_waiting_transitions -- --nocapture`
- `cargo test -p agenthub restart_run_keeps_linked_task_on_same_attempt_when_already_in_progress -- --nocapture`
