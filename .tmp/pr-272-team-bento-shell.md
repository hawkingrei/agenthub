## Summary

- refresh the Team workbench shell toward a Bento Box + Neo-Minimal visual language
- restyle shared Team panel, tab, sidebar, conversation, and Kanban surfaces without changing workflow behavior
- document the shell language and add a post-merge verification item

## What Changed

- updated shared Team Tailwind class presets in `web/src/ui/tailwind_classes.ts`
- refined Team workbench shell and header treatments in `web/src/pages/team_page.tsx`
- refined Team sidebar surfaces in `web/src/pages/team_sidebar.tsx`
- refined shared conversation and Kanban panel shells in
  - `web/src/pages/team_task_panel.tsx`
  - `web/src/pages/team_tasks_panel.tsx`
- added stable `data-team-surface` markers and focused panel tests in
  - `web/src/pages/team_tabs_bar.tsx`
  - `web/src/pages/team_panels.test.tsx`
- documented the design direction in `docs/features/frontend-design.md`
- added journal + TODO verification entry

## Validation

- `cd web && npx vitest run src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm run lint -- src/pages/team_page.tsx src/pages/team_sidebar.tsx src/pages/team_tabs_bar.tsx src/pages/team_tasks_panel.tsx src/pages/team_task_panel.tsx src/ui/tailwind_classes.ts`
- `cd web && npm run build`
- `git diff --check`

## MCP

- Chrome DevTools MCP baseline/regression check could not be completed in this environment because `chrome-devtools` returned `Transport closed` for `list_pages`.
