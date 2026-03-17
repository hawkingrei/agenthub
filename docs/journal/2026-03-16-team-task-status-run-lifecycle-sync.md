# Team task status follows run lifecycle

## Summary

Team task cards now follow linked run lifecycle changes automatically when the run input carries `task_id`.

## Backend changes

- `TeamManager::create_run` now promotes the linked task to `in_progress` as soon as a run submission is created.
- `TeamManager::complete_step` now marks the linked task `completed` when the run reaches `completed`.
- `TeamManager::cancel_run` now marks the linked task `canceled` when the run is explicitly canceled.
- `cancel_active_runs_on_startup` reopens linked tasks back to `open` after service-startup run cancellation so an infrastructure restart does not leave the Kanban board in a false canceled state.

## Status mapping

- `run submitted/working/input_required` -> `task in_progress`
- `run completed` -> `task completed`
- `run canceled` -> `task canceled`
- `run failed` -> no automatic task transition; the task stays `in_progress` because Team Kanban does not yet have a dedicated failed lane

## Validation

- `cargo test create_run_marks_linked_task_in_progress`
- `cargo test linked_run_completion_marks_task_completed`
- `cargo test linked_run_failure_keeps_task_in_progress`
- `cargo test cancel_run_marks_linked_task_canceled`
- `cargo test startup_cancellation_reopens_linked_task`
