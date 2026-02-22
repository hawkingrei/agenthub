# PR83 Review Hardening: Path Encoding And Callback Stability

## Background

PR #83 received review comments in two risk areas:

- Dynamic API path parameters (`teamId`, `runId`, `stepId`, `member/agent IDs`) were interpolated directly in `web/src/api.ts`.
- Several `useTeamActions` callbacks used by lifecycle hooks depended on high-frequency UI inputs, causing callback identity churn and avoidable effect re-runs.

## Scope

- Added a centralized path-segment encoder helper in `web/src/api.ts` and applied it to Team/Agent dynamic path segments.
- Hardened `useTeamActions` callback stability in `web/src/pages/team/use_team_actions.ts`:
  - `refreshSteps` now reads selected step state via ref.
  - `loadInbox` now reads inbox query state via ref.
  - `onLoadMoreRuns` now reads pagination guard state via ref.
- Expanded `web/src/pages/team/use_team_actions.test.tsx` with a non-token input churn case to assert lifecycle-facing callback identity stability.

## Key Decisions

1. Encode at API boundary, not call sites.

- Path-segment encoding is now centralized in `web/src/api.ts`.
- This avoids inconsistent per-caller sanitization and keeps endpoint construction rules explicit.

2. Keep callback identity stable for lifecycle wiring.

- Lifecycle hooks depend on callback identity in `useEffect` dependencies.
- Using refs for mutable UI inputs preserves latest values without rebuilding the callback on each keystroke/toggle.

3. Test what the lifecycle depends on.

- Added coverage for callback stability when non-token inputs change, aligned with the actual regression vector from review.

## Validation

Executed locally:

- `npm --prefix web run -s test -- src/pages/team/use_team_actions.test.tsx src/pages/team_page.runs.test.ts src/pages/team_panels.test.tsx`
- `npm --prefix web run -s lint`
- `npm --prefix web run -s build`

All commands passed.
