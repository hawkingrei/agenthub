# Team Workbench Bento Shell

## Summary

Refreshed the Team workbench shell toward a Bento Box plus Neo-Minimalist layout language.
The first pass keeps interaction contracts unchanged and focuses on the visual hierarchy of the
sidebar, workflow tabs, conversation shell, and Kanban shell.

## Goals

- Make the Team surface read as a set of intentional work boxes instead of one continuous panel wall.
- Keep the human workflow obvious: `# all` and `Kanban` stay primary, while navigation and tooling
  remain secondary.
- Preserve existing mobile and compact-pane behavior.

## Implementation Notes

- Upgraded shared Team panel, tab, and sidebar Tailwind presets in
  `web/src/ui/tailwind_classes.ts`.
- Refined Team workbench shell layout in `web/src/pages/team_page.tsx`.
- Refined Team sidebar box styling in `web/src/pages/team_sidebar.tsx`.
- Refined Kanban and shared conversation shells in
  `web/src/pages/team_tasks_panel.tsx` and `web/src/pages/team_task_panel.tsx`.
- Added stable `data-team-surface` markers for sidebar, workflow tabs, conversation, and Kanban
  shells to support regression checks without coupling tests to full class strings.

## Validation

- Pending post-edit Chrome DevTools MCP verification; current session still returns `Transport closed`.
