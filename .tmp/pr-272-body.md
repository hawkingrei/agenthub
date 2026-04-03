## Summary

- refresh the Team workbench shell toward a Bento Box + Neo-Minimal visual language
- restyle shared Team panel, tab, sidebar, conversation, and Kanban surfaces
- intentionally align the Team channel jump affordance with the new floating bottom-jump treatment
- document the shell language and add a post-merge verification item

## What Changed

- updated shared Team Tailwind class presets in `web/src/ui/tailwind_classes.ts`
- refined Team workbench shell and header treatments in `web/src/pages/team_page.tsx`
- refined Team sidebar surfaces in `web/src/pages/team_sidebar.tsx`
- refined shared conversation and Kanban panel shells in
  - `web/src/pages/team_task_panel.tsx`
  - `web/src/pages/team_tasks_panel.tsx`
- changed the Team channel thread affordance so long threads now expose only a floating `Jump to bottom` action; `Jump to top` was intentionally removed from the channel surface
- stabilized Team channel markdown rendering by preloading markdown assets and removing clipping that could truncate rich content
- extracted shared jsdom/Mantine test helpers and refreshed focused Team panel regressions in
  - `web/src/pages/team_panels.test.tsx`
  - `web/src/pages/team_member_acp_panel.test.tsx`
  - `web/src/test_utils/react_test_helpers.tsx`
- added stable `data-team-surface` markers and focused panel tests in
  - `web/src/pages/team_tabs_bar.tsx`
  - `web/src/pages/team_panels.test.tsx`
- documented the design direction in `docs/features/frontend-design.md`
- added journal + TODO verification entry

## Validation

- `cd web && npx vitest run src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npx vitest run src/pages/team_panels.test.tsx -t "TeamTaskPanel only renders markdown for the visible tail window until history is expanded" src/pages/team_member_acp_panel.test.tsx`
- `cd web && npm run lint -- src/pages/team_page.tsx src/pages/team_sidebar.tsx src/pages/team_tabs_bar.tsx src/pages/team_tasks_panel.tsx src/pages/team_task_panel.tsx src/ui/tailwind_classes.ts`
- `cd web && npm run lint -- src/pages/team_task_panel.tsx src/pages/team_panels.test.tsx src/pages/team_member_acp_panel.test.tsx src/test_utils/react_test_helpers.tsx`
- `cd web && npm run build`
- `git diff --check`

## MCP

- Chrome DevTools MCP baseline/regression check could not be completed in this environment because `chrome-devtools` returned `Transport closed` for `list_pages`.
