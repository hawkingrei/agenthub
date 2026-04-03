# Team UI Density And Tailwind Cleanup

## Summary

- Migrated more Team UI surfaces away from legacy global CSS and into explicit Tailwind utility classes.
- Compressed Team shared-thread cards for mobile and long-list usage.
- Collapsed non-pending permission review cards into compact status cards so timed-out and responded items stop dominating the conversation viewport.
- Refined the Team workbench shell toward a flatter, more document-like layout inspired by Notion, with tighter column spacing and weaker chrome around sidebar and main panels.

## Implementation

- `web/src/components/status_badge.tsx`
  - Replaced legacy `status-badge` CSS styling with inline Tailwind utility composition backed by existing status CSS variables.
- `web/src/pages/team_mailbox_panel.tsx`
  - Added explicit Tailwind layout classes for chat shell, member rail, message list, unread pill, message bubbles, and advanced mailbox block.
- `web/src/pages/team_steps_panel.tsx`
  - Added explicit Tailwind list/head/body classes for step records so `teams-step-*` no longer depends on `styles.css`.
- `web/src/pages/team_member_console_panel.tsx`
  - Added explicit Tailwind list/detail layout classes for member console event/detail blocks.
- `web/src/pages/team_task_panel.tsx`
  - Slimmed the shared-thread shell, item padding, seen-state badge, details button, and detail grid.
  - Added compact command-style body rendering for plain command messages.
  - Changed closed permission review cards to show `Command review` plus a one-line preview instead of reflowing the entire command into the title.
- `web/src/styles.css`
  - Removed dead legacy Team selectors that are now fully expressed by Tailwind classes:
    - `status-badge`
    - `team-status.status-badge*`
    - `team-create-*`
    - `team-skill-tag*`
    - `teams-worker-card`
    - `teams-chat-*`
    - `teams-message-*`
    - `teams-step-body`
- `web/src/pages/team_page.tsx`
  - Reduced the workbench gutter from `24px` to `16px`.
  - Flattened the main workbench surfaces, header shell, toolbar, and workspace shell by replacing glossy gradients and heavier shadows with lower-contrast borders and subtle white surfaces.
  - Softened the page background so the content reads more like a document canvas than a dashboard.
- `web/src/pages/team_sidebar.tsx`
  - Slimmed the sidebar shell, action buttons, active nav states, and section panels to match the flatter workbench treatment.
  - Kept selection affordances, but shifted them from glossy card styling to thin-border emphasis.
  - Reworked member information hierarchy so agent name stays primary, current work becomes the first supporting line, role/state collapse into a smaller humanized summary, and developer IDs move into a weaker tertiary line with shortened UUID display.
- `web/src/pages/team_task_panel.tsx`
  - Flattened the shared-thread activity shell, message bubbles, permission review cards, details disclosure block, seen popover, and jump button.
  - Reduced gradient usage in `# all` so command review and progress cards feel closer to document annotations than dashboard tiles.
- `web/src/pages/team_tasks_panel.tsx`
  - Flattened Kanban filter chrome, lane columns, task cards, detail panel, run cards, and debug disclosure.
  - Shifted lane counters and supporting callout panels to lighter neutral pills and low-contrast surfaces.
- `web/src/ui/tailwind_classes.ts`
  - Flattened the shared Team panel shell, toolbar, secondary controls, refresh button, and workflow tabs so the broader Team chrome matches the lighter workbench treatment.
- `web/src/components/input_dock.tsx`
  - Added explicit `id` and `name` attributes to the ACP input dock textarea to satisfy browser form-field checks.
- `web/src/input_dock_render.test.tsx`
  - Added a focused render assertion to keep the textarea `id/name` contract from regressing.
- `web/src/pages/team_task_panel.tsx`
  - Added explicit `id` and `name` to the `# all` composer textarea so the Team thread composer follows the same form-field contract.
- `web/src/pages/team_panels.test.tsx`
  - Added a regression assertion covering the Team thread composer `id/name` attributes.
  - Updated Team sidebar expectations to cover the new information hierarchy and developer-id presentation.

## Validation

- `cd web && npm run test -- src/pages/team_panels.test.tsx`
- `cd web && npm run test -- src/pages/team_page.smoke.test.tsx`
- `cd web && npm run lint`
- `make build-web`
- `cd web && npm run test -- src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx`
- `make build-web`
- `cd web && npm run test -- src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx`
- `make build-web`

## Chrome DevTools MCP

- Baseline before edits:
  - Checked `https://agenthub.hawkingrei.com/teams/276a2682-9ce7-4af5-aa6c-f12575d13c37` in mobile viewport `390x844`.
  - Confirmed Team shared-thread cards were still visually tall, with timed-out permission cards expanding long command strings.
- Regression after edits:
  - Reloaded the same live page after each `make build-web`.
  - Verified compact `Command review` labels appear for timed-out permission cards in the live snapshot.
  - Verified no new console errors were introduced; the only remaining console issue is the pre-existing form-field `id/name` warning.
  - Compared the live Team page against a neighboring Notion page to align toward weaker borders, flatter panels, and tighter gutters.
  - Measured the left/right workbench gap after refinement and confirmed it dropped from `24px` to `16px`.
  - Rechecked both `# all` and `Kanban` on `https://agenthub.hawkingrei.com/teams/276a2682-9ce7-4af5-aa6c-f12575d13c37` after the latest build.
  - Confirmed shared-thread cards and Kanban lane/task cards now use the flatter, lower-contrast treatment without introducing new console errors.
  - Identified the remaining console warning as the ACP input dock textarea missing `id/name`, then fixed it in the shared input dock component.
  - Rechecked the live sidebar against the neighboring Notion page and shifted agent cards toward a name-first, work-first hierarchy with quieter developer metadata.
