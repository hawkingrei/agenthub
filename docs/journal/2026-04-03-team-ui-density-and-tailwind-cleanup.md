# Team UI Density And Tailwind Cleanup

## Summary

- Migrated more Team UI surfaces away from legacy global CSS and into explicit Tailwind utility classes.
- Compressed Team shared-thread cards for mobile and long-list usage.
- Collapsed non-pending permission review cards into compact status cards so timed-out and responded items stop dominating the conversation viewport.

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

## Validation

- `cd web && npm run test -- src/pages/team_panels.test.tsx`
- `cd web && npm run test -- src/pages/team_page.smoke.test.tsx`
- `cd web && npm run lint`
- `make build-web`

## Chrome DevTools MCP

- Baseline before edits:
  - Checked `https://agenthub.hawkingrei.com/teams/276a2682-9ce7-4af5-aa6c-f12575d13c37` in mobile viewport `390x844`.
  - Confirmed Team shared-thread cards were still visually tall, with timed-out permission cards expanding long command strings.
- Regression after edits:
  - Reloaded the same live page after each `make build-web`.
  - Verified compact `Command review` labels appear for timed-out permission cards in the live snapshot.
  - Verified no new console errors were introduced; the only remaining console issue is the pre-existing form-field `id/name` warning.
