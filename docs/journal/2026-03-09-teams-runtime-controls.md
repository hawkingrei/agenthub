## Summary

Added explicit Team runtime controls to `/teams` so the UI matches the backend lifecycle split:

- Team runtime is member-owned and persistent.
- Run remains task/step execution state only.
- `/teams` now exposes `Start Team` and `Stop Team` in the workspace header.
- The `Runs` panel quick-start action is renamed to `Start Run` to avoid overloading "team" and "run".

## Frontend Changes

- Added `api.startTeam()` and `api.stopTeam()` plus `TeamRuntimeControlResponse`.
- Added `api.getTeamRuntime()` plus `TeamRuntimeRecord`.
- `/teams` now prefers the explicit backend `GET /api/teams/:id/runtime` read model for runtime badge and controls.
- `resolveTeamRuntimeStatus(...)` is now only a fallback path when the explicit Team runtime record is not yet loaded.
- Updated `/teams` workspace header to show:
  - runtime status badge
  - `Start Team`
  - `Stop Team`
- Updated workspace notice strip to include runtime state before run state.
- Added `team_runtime=...` to developer-mode workspace details.

## Validation

Recommended checks:

```bash
cd web && npx vitest run src/pages/team/page_helpers.test.ts src/pages/team_panels.test.tsx --pool=threads --maxWorkers=1
cd web && npm run lint
make build-web
```

Chrome DevTools MCP:

- Baseline should use `https://agenthub.hawkingrei.com/teams`
- Regression can use a local preview bundle before deployment when the domain is not yet updated

## Follow-up

- Add Team runtime controls to CLI and eventual `/teams` runtime summary panels in a consistent way.
