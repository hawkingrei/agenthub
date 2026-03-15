# Teams Selector Route Split

## Summary

- Split Team navigation into two route modes:
  - `/teams` is now the standalone Team Selector page.
  - `/teams/:team_id` is the team workbench detail page.
- Removed the assumption that the detail sidebar owns team switching.
- Added an in-workbench `Team Selector` return action so the selector remains the canonical entry point.
- Switched Team route navigation to in-app history updates so Team-local UI state can survive selector/detail transitions without full-page reloads.

## Implementation Notes

- `web/src/app.tsx`
  - tightened Team route detection to real `/teams` prefixes only;
  - introduced route-location state synced from `popstate` so Team route changes re-render inside the SPA without forcing `location.href` reloads.
- `web/src/pages/team_page.tsx`
  - kept selector and detail rendering paths separate;
  - changed Team route navigation to `history.pushState(...)` + `popstate`;
  - detail header now shows the selected team name and exposes a `Team Selector` return action.
- `web/src/pages/team_sidebar.tsx`
  - detail mode keeps the subject/operations rail but hides selector-only controls.
- `web/tests/e2e/team_page.e2e.ts`
  - updated helpers to enter teams through the selector page instead of assuming the detail sidebar owns cross-team switching;
  - updated delete-flow expectations to return to the selector page after removing the active team.

## Validation

- Targeted unit coverage:
  - `cd web && npx vitest run src/app.route_auth.test.ts src/pages/team_panels.test.tsx`
- Frontend static validation:
  - `cd web && npm run lint -- src/app.tsx src/app.route_auth.test.ts src/pages/team_page.tsx src/pages/team_sidebar.tsx src/pages/team_panels.test.tsx tests/e2e/team_page.e2e.ts`
  - `cd web && npm run build`
  - `cd web && npx playwright test tests/e2e/team_page.e2e.ts --list`

## Chrome DevTools MCP Notes

- Baseline before edits:
  - `https://agenthub.hawkingrei.com/teams` still rendered the old single-page Team workbench with `Team Selector` embedded in the left rail.
- Local regression check after edits:
  - used a fresh compiled preview at `http://127.0.0.1:4175/teams` with injected auth and minimal Team API mocks;
  - selector route showed `Team Selector`, `Team Directory`, and the team list as a standalone index page;
  - clicking `Alpha Desk` navigated to `/teams/team-local-alpha`, where the header switched to the team name and exposed a `Team Selector` button;
  - clicking `Team Selector` returned to `/teams` without falling back to the detail sidebar team list model.

## Follow-up

- After deployment, verify the selector/detail split on `agenthub.hawkingrei.com` and record deployed MCP evidence before closing the TODO item.
