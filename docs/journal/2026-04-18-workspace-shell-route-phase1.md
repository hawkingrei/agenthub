# Workspace Shell Route Phase 1

- Date: 2026-04-18

## Summary

Begin the unified workspace-shell rollout by changing the top-level Agents workbench from an
`Agents`-named shell toward a canonical `Workspace` shell, while preserving the existing Team and
Agent inner surfaces.

## Changes

- Add `/workspace` as a canonical alias for the existing Agent workbench shell.
- Update route-selection helpers so the root workbench aliases (`/` and `/workspace`) share the
  same route kind and post-auth redirect behavior.
- Update the workbench header menu to use `Workspace` as the primary shell entry instead of
  `Agents`.

## Validation

- `cd web && npm run test -- vite.config.test.ts src/workbench_header_menu.test.tsx src/app_route_selection.test.ts src/app.route_auth.test.ts`
- Chrome DevTools MCP should confirm that the top menu now renders `Workspace` and still exposes
  `Teams` as a sibling shell entry.
