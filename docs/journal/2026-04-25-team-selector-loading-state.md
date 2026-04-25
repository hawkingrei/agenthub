## Team Selector Loading State

- Split Team selector loading from the true empty state so slow `/api/teams` refreshes do not flash `No teams yet` on first load.
- Preserve the existing Team list while a refresh is in flight; only show the empty-state copy after the refresh settles with zero teams.
- Added smoke coverage for the selector route loading state and updated the selector panel component tests.

### Validation

- `cd web && pnpm exec vitest run src/pages/team/team_selector_panel.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm run build`
