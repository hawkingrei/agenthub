# Team Panel Coverage Boost

## Summary

Add focused interaction tests for extracted Team panel components to improve web coverage and prevent regressions in panel callback wiring.

## Background

After splitting Team Workbench UI into multiple panel components, coverage reports showed several `team_*_panel.tsx` files with low or zero direct line coverage. Most logic is callback-driven UI wiring, so lightweight jsdom interaction tests are sufficient and cheaper than end-to-end-only validation.

## Scope

- `web/src/pages/team_panels.test.tsx`
- `docs/todo.md`

## Key Decisions

1. Add one consolidated test file for panel-level interaction coverage instead of many tiny files to keep setup overhead low.
2. Cover callback contracts and conditional rendering branches for:
   - `TeamSidebar`
   - `TeamRunPanel`
   - `TeamStepsPanel`
   - `TeamEventsPanel`
   - `TeamOverviewPanel`
   - `TeamMemberConsolePanel`
   - `TeamMailboxPanel`
3. Keep tests at component boundary (no API mocking layer) to avoid coupling to data-fetch orchestration in `team_page.tsx`.

## Validation

Executed (2026-02-19):

```bash
npm --prefix web run test -- src/pages/team_panels.test.tsx
npm --prefix web run test:coverage
npm --prefix web run lint
npm --prefix web run build
```

Coverage snapshot after this change:

- `web` line coverage: `49.98% (2578/5158)`
- `team_*_panel.tsx` extracted panel files covered at `100%` line coverage.
