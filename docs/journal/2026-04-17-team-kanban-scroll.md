# Team Kanban Scroll

## Summary

Restored vertical scrolling for the Team Kanban surface after the Team workbench
layout refactors left the panel inside an `overflow-hidden` shell without its own
scroll container.

## Changed

- `web/src/pages/team_tasks_panel.tsx`
  - made the Kanban surface itself vertically scrollable with contained overscroll
  - kept the existing workbench shell layout unchanged so conversation and other
    Team surfaces do not inherit a broader scroll behavior change
- `web/src/pages/team_panels.test.tsx`
  - added regression coverage to ensure the Kanban surface keeps vertical scroll
    classes

## Root Cause

- `web/src/styles.css` keeps `html`, `body`, and `.app` on `overflow: hidden`
- `web/src/ui/tailwind_classes.ts` defines `TEAM_PANEL_CARD_CLASS` with
  `overflow-hidden`
- unlike the conversation surface, `TeamTasksPanel` did not create an inner
  `overflow-y-auto` container, so long Kanban content was clipped instead of
  scrollable

## Validation

- `cd web && npm run test -- vite.config.test.ts src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx`

## Notes

- Chrome DevTools MCP validation was attempted before and after the change, but the
  local MCP transport closed before a page session could be established. The code
  change therefore relies on targeted frontend regression tests for this patch.
