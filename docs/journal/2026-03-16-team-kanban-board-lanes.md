# 2026-03-16 Team Kanban Board Lanes

## Summary

- Upgraded the Team `Kanban` workspace from a filtered task list into a status-column board.
- Kept the existing create-task and compile-preview flows intact to avoid widening the backend/API surface.
- Preserved the current `tasks` tab state model while changing the visual model to board lanes.

## Context

The previous sidebar change promoted `Kanban` into a first-class Team entry, but the content area
was still a classic `list + detail` panel:

- one filtered vertical task list on the left;
- one detail / compile-preview panel on the right;
- no visual grouping by task status.

That meant the IA said "Kanban" while the actual planning surface still behaved like a filtered task
table. The next step was to make the workspace materially closer to a GitHub Projects board without
adding drag-and-drop or status mutation APIs yet.

## What Changed

- `web/src/pages/team_tasks_panel.tsx`
  - replaced the left-side single task list with a horizontally scrollable board made of four status
    lanes:
    - `Open`
    - `In progress`
    - `Completed`
    - `Canceled`
  - kept the existing segmented status filter, but it now controls lane visibility instead of
    filtering one flat list;
  - moved the workspace framing to `Board lanes`, including lane count and visible task count;
  - kept task selection semantics unchanged: clicking a card still drives the detail /
    compile-preview panel below.
- `web/src/pages/team_panels.test.tsx`
  - updated the existing task-panel test to assert the new board framing text and lane labels while
    preserving create/filter/compile-preview interaction coverage.

## Validation

- `cd web && npm run test -- src/pages/team_panels.test.tsx`
- `cd web && npm run lint -- src/pages/team_tasks_panel.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run build`

## Chrome MCP Verification

- Baseline was captured on a temporary uncommitted local preview page that rendered the pre-change
  `TeamTasksPanel` directly under Vite:
  - the page still showed `Task list`;
  - all tasks were stacked in one vertical list;
  - the status segmented control filtered that single list.
- Post-edit regression used the same temporary preview path, then removed the preview before
  finishing the change:
  - Chrome DevTools MCP confirmed four visible lanes: `Open`, `In progress`, `Completed`,
    `Canceled`;
  - task cards now render inside the matching status lane instead of one shared list;
  - the detail / compile-preview panel still renders for the selected task below the board.
