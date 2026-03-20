# Team Runtime Freshness Tightening

## Summary

- Updated `Teams` runtime controls so `Start Team` / `Stop Team` apply the returned runtime state to the selected Team immediately instead of waiting for background `refreshTeams()` / `refreshAgents()` calls to finish.
- Added a lightweight Team runtime watcher that rechecks the selected Team roughly every minute while configured members are active, reducing stale runtime badges when no other UI activity occurs.
- Added focused web tests for the runtime polling hook and kept existing Team panel regressions green.

## Implementation Notes

- `web/src/pages/team_page.tsx`
  - Apply optimistic runtime updates immediately after `api.startTeam(...)` / `api.stopTeam(...)` resolves.
  - Move `refreshTeams()` / `refreshAgents()` into best-effort background sync so the visible runtime chip is no longer blocked by slower list refreshes.
  - Enable runtime watching only when the selected Team has configured members and is either running/degraded or currently starting.
- `web/src/pages/team/use_team_runtime_effects.ts`
  - New hook that polls `refreshTeamRuntime(teamId)` every 60 seconds while enabled.
- `web/src/pages/team/use_team_runtime_effects.test.tsx`
  - Added coverage for minute-level polling, disabled mode, and error forwarding without breaking later polls.

## Validation

- `cd web && npx vitest run src/pages/team/use_team_runtime_effects.test.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run lint -- src/pages/team/use_team_runtime_effects.ts src/pages/team/use_team_runtime_effects.test.tsx src/pages/team_page.tsx src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm run build`

## Follow-up

- Continue slimming the Team workspace utility navigation: move `Runs / Advanced` into the top-right utility area and keep regression coverage for the `Agent ACP` entry path.
