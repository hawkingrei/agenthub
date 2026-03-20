## Summary

Aligned Team Kanban with a five-stage task flow by introducing `in_review` as a first-class task status.

## Changes

- Added `in_review` to the shared Team task status domain contract.
- Updated Team task status parsing/serialization and API validation to accept `in_review`.
- Changed automatic linked-task completion behavior so a successful run now moves the task into `in_review` instead of `completed`.
- Updated the Team Kanban board to render an `In review` lane and expose review-specific actions:
  - `Send to review`
  - `Needs changes`
  - `Approve`

## Validation

- `cargo test linked_run_completion_marks_task_in_review`
- `cargo test teams_api_updates_task_status_and_rejects_invalid_values`
- `cd web && npx vitest run src/pages/team_panels.test.tsx`
- `cargo fmt --all --check`
