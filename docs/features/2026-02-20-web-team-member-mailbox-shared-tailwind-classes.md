# Web Team Member/Mailbox Shared Tailwind Classes

## Background

`TeamMemberConsolePanel` and `TeamMailboxPanel` still duplicated panel shell,
button, and input Tailwind class strings after phase-3 extraction. This made
style maintenance harder and increased drift risk.

## Scope

- `web/src/pages/team_member_console_panel.tsx`
- `web/src/pages/team_mailbox_panel.tsx`
- `web/src/ui/tailwind_classes.ts`
- `docs/todo.md`

## Key Decisions

1. Reuse shared Team panel constants from `tailwind_classes.ts`:
   - card shell and toolbar;
   - primary/secondary buttons;
   - shared input styles.
2. Keep existing semantic class names and DOM structure for test stability:
   - `teams-chat-members`, `teams-chat-messages`, `teams-message-panel`,
     `teams-event-list`, `teams-event-head`.
3. Add local utility combinations for readability only (member button active
   state, conversation empty text, list item surfaces) without changing behavior.
4. Keep interaction flow unchanged:
   - member select / refresh / load older
   - chat send / ack / jump-to-bottom
   - advanced JSON send/template/inbox controls

## Validation Evidence (local)

- `npm --prefix web run test -- src/pages/team_panels.test.tsx`
- `npm --prefix web run lint`
- `npm --prefix web run build`

## Notes

- This change is style-layer maintainability refactor only; no API/reducer logic
  changed.
