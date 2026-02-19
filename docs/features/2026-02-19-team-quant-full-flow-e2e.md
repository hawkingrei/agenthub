# Team Quant Full Flow E2E

## Background

Team workbench already had creation/mailbox/list tests, but no dedicated scenario covering
"quant team" end-to-end lifecycle from team creation to run launch with role-specific member
responsibilities.

## Scope

- `web/tests/e2e/team_page.e2e.ts`

## Key Decisions

1. Add a new Playwright scenario:
   - create a quant team via Team Forge manual spec;
   - leader handles planning/resource control;
   - worker-1 handles portfolio optimization;
   - worker-2 handles crypto algo trading;
   - launch `Create Run` and assert run/snapshot visibility.
2. Update duplicate-assignment regression test to trigger duplicate state through
   leader-stage reassignment after worker selection (matches current UI constraint behavior).

## Validation Evidence (2026-02-19)

- Command:
  - `cd web && PLAYWRIGHT_NO_WEBSERVER=1 npm run e2e -- team_page.e2e.ts`
- Result:
  - `9 passed (22.7s)` including:
    - `team quant workflow creates team and launches run`
    - existing Team forge/mailbox/delete/run-list regressions.
