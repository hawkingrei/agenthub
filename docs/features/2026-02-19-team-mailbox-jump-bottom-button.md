# Team Mailbox Jump To Bottom Button

## Background

Team mailbox IM mode exposed `auto_follow=on/off`, but there was no explicit `Jump to bottom`
action in the chat header. This made it easy to lose the latest context after scrolling up.

## Scope

- `web/src/pages/team_mailbox_panel.tsx`
- `web/src/pages/team_page.tsx`
- `web/src/styles.css`
- `web/src/pages/team_panels.test.tsx`

## Key Decisions

1. Add an explicit `Jump to bottom` button in Team mailbox chat header.
2. Wire the button to:
   - set `chatStickToBottom=true`,
   - scroll conversation to bottom in `requestAnimationFrame`,
   - mark current conversation as seen.
3. Keep the button disabled when there are no chat messages.

## Validation Evidence (2026-02-19)

- Command:
  - `cd web && npm run test -- src/pages/team_panels.test.tsx`
- Result:
  - `src/pages/team_panels.test.tsx (7 tests)` passed.

- Command:
  - `cd web && npm run build`
- Result:
  - Vite production build passed.
