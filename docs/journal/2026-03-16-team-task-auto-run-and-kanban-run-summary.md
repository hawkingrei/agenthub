# 2026-03-16 Team Task Auto Run And Kanban Run Summary

## Summary

- Team task creation now auto-starts a linked run when the task is an executable work item and the
  team already has configured members.
- Shared-thread bootstrap tasks (`all`) stay task-only and do not create runs.
- Kanban task detail now treats linked runs as the primary execution view; compile preview remains
  available only as a developer/debug affordance.

## Why

The previous Team flow still exposed an older mental model:

- create a task
- manually compile it into a run payload
- manually create a run from that payload

That conflicts with the intended Team semantics:

- `task` is the agent-facing work object
- agents should execute tasks automatically
- `run` is the execution record and final summary for one attempt

## Backend changes

- `src/api/teams.rs`
  - `POST /api/teams/:id/tasks` now auto-creates a linked run for normal executable tasks.
  - Shared-thread tasks are excluded from auto-run bootstrapping by `bootstrap_kind=shared_thread`.
  - `TeamTaskDetailResponse` now includes `latest_run`.
- `src/team/manager.rs`
  - `TeamRunRecord` responses now hydrate an optional `summary` string derived from the latest
    step output/error, with stable fallbacks for completed/failed/canceled runs.
  - Added `get_latest_run_for_task` so task surfaces can fetch the newest linked run directly.
- `src/team/orchestrator.rs`
  - Dispatch prompts now include a task brief extracted from run input (`task_title`,
    `task_list`, `acceptance_criteria`, `deadline`) so auto-created runs remain actionable without
    a manual compile-preview handoff.

## Frontend changes

- `web/src/pages/team_tasks_panel.tsx`
  - Reframed the task detail panel around `Latest run` and `Previous runs`.
  - Added `Open Run` navigation from Kanban task detail into the run workspace.
  - Moved compile-preview controls into a developer-only debug section.
- `web/src/pages/team_page.tsx`
  - Applies auto-created runs returned from task creation into the local run browser state so the
    task and run surfaces stay in sync immediately.
  - Updated the top-level Kanban workspace description so the page-level copy also reflects the new
    task/run model instead of the legacy compile-preview wording.
- `web/src/pages/team/run_helpers.ts`
  - Added task-id extraction helpers for filtering linked runs from the run list.

## Validation

- `cargo test team_task_api_creates_lists_and_redacts_context`
- `cargo test team_task_api_keeps_shared_thread_tasks_without_auto_run`
- `cargo test dispatch_once_injects_actor_runtime_and_supports_inbox_ack_flow`
- `cd web && npx vitest run src/pages/team_panels.test.tsx src/pages/team/run_helpers.test.ts`
- `cd web && npm run lint -- src/pages/team_tasks_panel.tsx src/pages/team_page.tsx src/pages/team_panels.test.tsx src/pages/team/run_helpers.ts src/pages/team/run_helpers.test.ts src/api.ts`
- `cd web && npm run build`

## Chrome MCP

- Baseline checked on `https://agenthub.hawkingrei.com/teams` before edits:
  - current domain still shows the pre-change Teams surface, including the old compile-preview-led
    task detail model.
- Follow-up live check on `https://agenthub.hawkingrei.com/teams/<team_id>` after partial deploy:
  - task-panel empty-state copy had already switched to linked-run wording, but the page-level
    Kanban description still used the old compile-preview/create-run copy.
  - This change aligns that remaining page-level copy with the task/run execution model.
- Post-edit regression on the same domain is blocked until this change is deployed because the user
  required domain-only verification and local-page verification was intentionally not used.
