# 2026-03-16 Team Sidebar Kanban Entry

## Summary

- Promoted `Kanban` into a first-class Team sidebar entry beside `all`.
- Grouped `leader` and `worker-*` entries strictly under `Agents`.
- Removed the left-rail scope switch so the Team workbench reads as one stable navigation tree.

## Context

The earlier Team subject-rail iteration moved toward Slock's navigation model, but the final left
rail still exposed two extra abstractions that no longer matched the intended product language:

- a scope switch between `Channels & Agents` and `Operations`;
- `Tasks` as an internal workspace tab instead of a stable planning surface.

The current Team IA should reflect three distinct object types directly:

- Team-level shared work surfaces: `all`, `Kanban`;
- Team members: `Agents -> leader / worker-*`;
- Operational tools: `Runs`, `Advanced`.

This keeps the Team workbench closer to a GitHub Projects style split between shared discussion and
planning surfaces, without turning tasks into a separate channel system.

## What Changed

- `web/src/pages/team_sidebar.tsx`
  - added a dedicated `onSelectKanban` callback;
  - removed the sidebar scope switch and the old `Channels` / `Operations` section split;
  - rendered the selected-team rail in one fixed order:
    - `all`
    - `Kanban`
    - `Agents`
    - `Runs`
    - `Advanced`
  - kept agent rows opening member mailbox/thread context while preserving status badges and pending
    inbox counts.
- `web/src/pages/team_page.tsx`
  - renamed workspace labels from `Conversation` / `Tasks` to `all` / `Kanban`;
  - wired the new sidebar `Kanban` entry back to the existing `tasks` tab state;
  - updated workspace copy to describe the shared thread and task board with the new wording.
- `web/src/pages/team_tasks_panel.tsx`
  - renamed the panel title from `Tasks` to `Kanban`.
- `web/src/pages/team_panels.test.tsx`
  - updated sidebar interaction coverage for direct `Kanban`, `Runs`, and `Advanced` entries;
  - kept coverage for agent grouping and shared-thread selection;
  - updated task-panel expectations to the `Kanban` label.

## Validation

- `cd web && npm run test -- src/pages/team_panels.test.tsx src/workbench_connection_badge.test.tsx src/acp_panel.test.tsx`
- `cd web && npm run lint -- src/pages/team_sidebar.tsx src/pages/team_page.tsx src/pages/team_tasks_panel.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run build`

## Chrome MCP Verification

- Baseline on the normal local Team route (`http://127.0.0.1:4175/teams/team-1`) was blocked by the
  missing API backend. The page fell into the expected bootstrap failure state (`Unexpected token
  '<'`, `Team not found`), so populated Team-workbench inspection was not reliable there.
- Post-edit structural verification used a temporary uncommitted local preview page that rendered
  `TeamSidebar` directly under Vite, then removed that preview before finishing the change.
- Chrome DevTools MCP confirmed the intended order and grouping in the rendered sidebar:
  - top-level entries `all` and `Kanban`;
  - `Agents 3` group containing `leader`, `worker-1`, and `worker-2`;
  - separate `Runs` and `Advanced` entries below the agent list.
