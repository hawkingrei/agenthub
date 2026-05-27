# Team Task-First P0 Verification

## Summary

Closed the remaining `P0` verification gap for the task-first Team model by locking canonical task
creation against implicit run creation and recording the focused validation set that already covers
priority, assignment, and prompt framing.

## Background

The task-first Team direction was already implemented and documented, but `docs/todo.md` still kept
one `P0` verification item open. The remaining missing proof was an explicit regression that
canonical task creation does not materialize a new run behind the scenes when a team already has
other run state.

## Scope

- Added a regression assertion to the internal Team control integration test so task creation must
  leave `team_runs` count unchanged.
- Reused the existing focused negative tests for explicit `priority` and `assigned_member_id`
  requirements.
- Reused the prompt contract test that keeps `task` primary and `run` / `step` framed as
  execution/debug artifacts.

## Key Decisions

- Verified the contract through focused integration tests instead of reopening wider Team UI or
  runtime matrices.
- Treated prompt-contract coverage as the right guardrail for the docs/skills half of this TODO,
  because the managed Team prompts are the runtime-facing contract that must keep task-first
  semantics stable.
- Removed the completed `P0` item from `docs/todo.md` instead of leaving stale open-work text in
  place.

## Validation

```bash
cargo fmt --all --check
cargo test -p agenthub-team-prompts prompt_templates_keep_required_contract_lines -- --nocapture
cargo test -p agenthub internal_grpc_team_context_and_task_controls_are_wire_compatible --target-dir /private/tmp/agenthub-task-first-p0-target -- --nocapture
cargo test -p agenthub internal_grpc_team_task_create_requires_priority --target-dir /private/tmp/agenthub-task-first-p0-target -- --nocapture
cargo test -p agenthub internal_grpc_team_task_create_requires_assigned_member_id --target-dir /private/tmp/agenthub-task-first-p0-target -- --nocapture
```

## Follow-Ups

- Keep future Team task/create regression work focused on explicit ownership and execution
  boundaries rather than reopening the removed backend auto-orchestrator path.
