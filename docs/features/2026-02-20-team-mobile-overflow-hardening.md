# Team Mobile Overflow Hardening

## Background

After the Team UI migration, mobile usage exposed a practical regression: long member IDs,
run/member metadata, and JSON payload blocks could expand horizontal layout space and make
touch interaction difficult on narrow screens.

## Scope

- `web/src/ui/tailwind_classes.ts`
- `web/src/pages/team_page.tsx`
- `web/src/pages/team_events_panel.tsx`
- `web/src/pages/team_member_console_panel.tsx`
- `web/src/pages/team_mailbox_panel.tsx`
- `web/src/styles.css`
- `web/tests/e2e/team_page.e2e.ts`

## Key Decisions

1. Make Team panel toolbars mobile-safe by default:
   - shared toolbar/actions classes now use `flex-wrap`.
2. Add a shared Team `pre` rendering class:
   - use wrapping + controlled horizontal scroll (`whitespace-pre-wrap`, `break-words`, `overflow-x-auto`) to keep long payloads readable.
3. Harden Team shell/header for mobile viewport:
   - Team page root uses runtime viewport minimum height (`--agenthub-vh`).
   - header session group can wrap and username is truncated on small screens.
4. Prevent long IDs from stretching cards:
   - apply ellipsis-safe constraints to Team list/member row text (`team-name`, `team-id`, `team-member-row-id`).
5. Strengthen mobile regression coverage:
   - mobile E2E case now uses intentionally long member IDs.
   - assert no horizontal overflow (`scrollWidth - clientWidth <= 1`).
6. Keep Team run tabs readable on narrow screens:
   - introduce Team-only `team-tab-bar` class to force `nowrap` while preserving horizontal scroll.
   - sync small-screen session gap rule to include `.team-session` after header class rename.

## Validation Evidence (2026-02-20)

- `npm --prefix web run test -- src/pages/team_panels.test.tsx src/pages/team_page.runs.test.ts`
- `PLAYWRIGHT_MINIMAL_RUNTIME=1 PLAYWRIGHT_NO_WEBSERVER=1 PLAYWRIGHT_PORT=4174 npm --prefix web run e2e -- --grep "team page keeps single-column proportions on mobile viewport" tests/e2e/team_page.e2e.ts`
- `PLAYWRIGHT_NO_WEBSERVER=1 npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts --grep "team page keeps single-column proportions on mobile viewport"`
- `PLAYWRIGHT_NO_WEBSERVER=1 npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts --grep before_created_at`
- `npm --prefix web run lint`
- `npm --prefix web run build`

## Notes

- This hardening focuses on mobile layout safety/readability and does not change Team run/mailbox business logic.
- Real-device iOS/Android checks are still recommended for keyboard + nested scroll ergonomics under very long live payload streams.
