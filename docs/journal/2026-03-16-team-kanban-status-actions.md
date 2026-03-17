# 2026-03-16 Team Kanban Status Actions

## Summary

- Added end-to-end Team task status updates so `Kanban` cards are no longer read-only.
- Kept the interaction deliberately lightweight: explicit card actions instead of drag-and-drop.
- Reused the existing Team task resource path with a small `PATCH` payload.

## Context

After converting the Team `Kanban` workspace into four status lanes, the board still lacked one key
property of a planning surface: cards could not change lanes.

The next smallest useful slice was:

- keep the current Team task domain model;
- add one status-update API;
- expose clear per-card actions (`Start`, `Complete`, `Cancel`, `Reopen`) in the board UI.

This avoids expanding the diff into drag/drop pointer handling, touch behavior, keyboard reordering,
and lane-hover state before the core status-mutation path is proven.

## What Changed

- `src/team/manager.rs`
  - added `update_task_status`, updating `team_tasks.status` and `updated_at`.
- `src/api/teams.rs`
  - added `PATCH /api/teams/:id/tasks/:task_id`;
  - validated incoming `status` against the canonical Team task status enum values.
- `web/src/api.ts`
  - added `api.updateTeamTask(...)`.
- `web/src/pages/team_page.tsx`
  - wired a new `onUpdateTaskStatus` callback that patches the task, refreshes local task ordering,
    and preserves selection on the updated card.
- `web/src/pages/team_tasks_panel.tsx`
  - added explicit status-action buttons to each card:
    - `Start`
    - `Complete`
    - `Cancel`
    - `Reopen`
  - mirrored those actions in the selected-task detail panel for the focused card.
- tests
  - `src/team/manager/tests.rs`
  - `src/api/teams/tests_core.rs`
  - `src/api/teams/tests_router.rs`
  - `web/src/pages/team_panels.test.tsx`

## Validation

- `cargo test task_status`
- `cargo test teams_router_http_contract`
- `cd web && npm run test -- src/pages/team_panels.test.tsx`
- `cd web && npm run lint -- src/pages/team_tasks_panel.tsx src/pages/team_panels.test.tsx src/pages/team_page.tsx src/api.ts`
- `cd web && npm run build`

## Chrome MCP Verification

- Baseline came from the temporary local `TeamTasksPanel` preview used in the previous board-lane
  step: cards were visible in lanes, but there were no card-level status actions.
- Post-edit regression used the same temporary preview path, then removed that preview before
  finishing the change:
  - Chrome DevTools MCP confirmed card-level status actions now render in each lane, for example
    `Start` / `Cancel` in `Open` and `Reopen` / `Complete` / `Cancel` in `In progress`;
  - the selected-task detail panel also exposes matching status actions;
  - the rest of the `Kanban` surface (`Board lanes`, compile preview, create run actions) remains
    present.
