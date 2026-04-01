# Team Workbench UX Alignment

## Summary

This change closes the current frontend workbench gaps around Team task-first UX, narrow-screen
layout, and resume-time refresh behavior.

## What Changed

- promoted `Conversation` (`# all`) and `Kanban` as explicit primary workflow surfaces inside the
  workspace, not only inside the Team rail
- removed direct human task creation controls from the Kanban panel and replaced them with guidance
  that canonical Team tasks are leader/runtime-managed
- reordered the Team rail so `# all` appears before `Kanban` and added intent copy for each
  surface
- tightened Team runtime, Kanban, and shared conversation refresh so they immediately recheck on:
  - window focus
  - document visibility restore
  - network reconnect
- changed narrow-screen Team detail layout into two panes:
  - Team rail pane
  - workspace pane
  switched by the existing header toggle instead of stacking both panes into one long page

## Validation

Ran:

```bash
cd web && npx vitest run \
  src/pages/team/use_team_runtime_effects.test.tsx \
  src/pages/team/use_team_task_effects.test.tsx \
  src/pages/team/use_team_conversation_effects.test.tsx \
  src/pages/team_panels.test.tsx \
  src/pages/team_page.smoke.test.tsx

cd web && npm run lint
cd web && npm run build
```

Agent-browser baseline notes:

- opened `https://agenthub.hawkingrei.com/teams`
- captured Team Selector and one Team detail route snapshot
- confirmed the current deployed page exposes `# all`, `Kanban`, member rail entries, and the
  workspace header in both desktop and narrow viewport snapshots before the local edits

Limitations:

- Chrome DevTools MCP was unavailable during this change (`Transport closed`), so MCP baseline and
  MCP regression capture could not be completed
- deployed browser regression for the changed code path was not possible before merge because the
  edited frontend was not yet running on the deployed environment

## Follow-up

- public `POST /api/teams/:id/tasks` still exists even though the normal UI no longer exposes
  direct human task creation; tighten that API contract separately if we want full task-first
  enforcement across public entry points
