# Team Run Panel Readability Layout

## Background

Team workbench feedback indicated that core controls felt visually stacked and hard to scan,
especially when operators had to switch quickly between run creation and run browsing.

## Scope

- `web/src/pages/team_run_panel.tsx`
- `docs/todo.md`

## Key Decisions

1. Split top Team run workbench into clearer zones:
   - keep member health as an independent block (`Team Health`);
   - place run creation and run browsing into a responsive 2-column grid on large screens.
2. Keep mobile behavior safe by design:
   - grid collapses to single column automatically under `xl`.
3. Preserve existing interaction contracts:
   - keep original class hooks (`teams-run-create`, `teams-run-list`, `team-item`) for tests and behavior continuity.
   - no change to API calls, reducer wiring, or run state transitions.

## Validation Evidence (2026-02-20)

- `npm --prefix web run test -- src/pages/team_panels.test.tsx src/pages/team_page.runs.test.ts`
- `npm --prefix web run lint`
- `npm --prefix web run build`
- `PLAYWRIGHT_MINIMAL_RUNTIME=1 PLAYWRIGHT_NO_WEBSERVER=1 PLAYWRIGHT_PORT=4174 npm --prefix web run e2e -- --grep "team page keeps single-column proportions on mobile viewport" tests/e2e/team_page.e2e.ts`

## Notes

- This refinement is intentionally structural/visual only; business logic remains unchanged.
