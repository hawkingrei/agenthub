# Web Tailwind UI Phase-4: Team Member Console + Mailbox Panels

## Background

After phases 1-3, the remaining high-interaction Team workbench surfaces are
`TeamMemberConsolePanel` and `TeamMailboxPanel`. These views include frequent
user actions (member switching, chat ack/send, advanced mailbox controls), so we
keep migration UI-only and preserve behavior.

## Scope

- `web/src/pages/team_member_console_panel.tsx`
- `web/src/pages/team_mailbox_panel.tsx`
- `docs/todo.md`

## Key Decisions

1. Keep all interaction handlers unchanged:
   - member selection/refresh/load-older
   - chat send/ack/jump-to-bottom
   - advanced message send/template/inbox query controls
2. Layer Tailwind utility classes for:
   - panel shell and toolbar
   - select/input/textarea focus states
   - conversation/member list surfaces
   - button hierarchy (primary/secondary)
3. Preserve existing semantic class names and structure to minimize regression
   risk and keep panel tests stable.

## Validation Evidence (local)

- Focused panel tests:
  - `npm --prefix web run test -- src/pages/team_panels.test.tsx`
- Lint:
  - `npm --prefix web run lint`
- Build:
  - `npm --prefix web run build`

## Follow-up Validation

- Manual desktop/mobile checks in `/teams`:
  - member selector and profile detail readability
  - mailbox member-list active/unread states
  - conversation bubble readability and jump-to-bottom behavior
  - compose box + advanced controls disabled/loading state visuals

## Notes

- This phase does not change mailbox payload semantics, ack flow, or polling
  logic; all changes are style-layer only.
