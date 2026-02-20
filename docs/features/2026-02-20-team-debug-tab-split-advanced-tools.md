# Team Debug Tab Split For Advanced Step/Mailbox Tools

## Background

The Team run workbench mixed high-frequency tabs (`Overview`, `Events`, `Steps`, `Mailbox`, `Member Console`) with lower-frequency engineering controls (step submit/action mutation and raw mailbox JSON operations).
This increased visual density and made quick task routing slower.

## Scope

- `web/src/pages/team/state.ts`
- `web/src/pages/team_page.tsx`
- `web/src/pages/team_steps_panel.tsx`
- `web/src/pages/team_mailbox_panel.tsx`
- `web/src/pages/team_panels.test.tsx`
- `web/tests/e2e/team_page.e2e.ts`
- `docs/todo.md`

## Key Decisions

1. Add a dedicated Team top-level `Debug` tab:
   - Keep primary work tabs focused on operational flow.
   - Move engineering/debug-oriented controls under Debug.
2. Introduce per-function Debug tags:
   - `Step Ops`: render only step submit/action controls.
   - `Mailbox Raw`: render only raw message send + inbox query controls.
3. Split `TeamStepsPanel` into explicit render modes:
   - `list_only` for the normal `Steps` tab.
   - `controls_only` for `Debug -> Step Ops`.
   - `full` remains default for compatibility.
4. Split `TeamMailboxPanel` into explicit render modes:
   - `full` for the normal `Mailbox` tab (conversation-first).
   - `advanced_only` for `Debug -> Mailbox Raw`.
5. Keep interaction contracts unchanged:
   - Existing handlers/callback wiring are preserved.
   - Only view partitioning and discoverability are adjusted.
6. Simplify Team output layout to avoid panel stacking/overlap:
   - Render tab content under one `teams-output-stack` container.
   - Keep Debug tool tags in an opaque, elevated strip (`z-index` + solid background).
   - Remove fixed global card min-height pressure (`.card` `min-height: 0`) to reduce accidental vertical overlap.

## Validation Evidence (2026-02-20)

- `npm --prefix web run lint`
- `npm --prefix web run test -- src/pages/team_panels.test.tsx src/pages/team_page.runs.test.ts`
- `PLAYWRIGHT_NO_WEBSERVER=1 npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts --grep "team mailbox IM mode supports conversation focus, unread, auto-follow and advanced controls"`

## Notes

- The Team run workflow semantics are unchanged.
- This change primarily reduces UI crowding by separating day-to-day operations from debug tooling.
