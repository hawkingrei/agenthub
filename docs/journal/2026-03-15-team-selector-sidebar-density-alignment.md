## Summary

- Reworked the Teams sidebar selector to follow a denser workbench-style navigation model.
- Kept AgentHub's own palette and component language instead of copying the reference visual skin.
- Preserved existing team actions and filters while making the team index a first-class sidebar section.

## What Changed

- Replaced the oversized team switcher button with a compact `Team Selector` header plus action controls.
- Moved team selection into a persistent `Teams N` section with inline filter and denser team rows.
- Added compact selection affordance (`Current`) and summary metadata (`members`, `active`, `idle`, `missing`) per team row.
- Kept the existing scope switch, but aligned section labels with a navigator rhythm:
  - `Teams N`
  - `Channels 1`
  - `Agents N`
  - `Operations N`
- Restyled the Teams sidebar surface from heavy black brutalist blocks to the existing neutral AgentHub workbench palette.

## Validation

- `cd web && npx vitest run src/pages/team_panels.test.tsx`
- `cd web && npm run lint -- src/pages/team_sidebar.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run build`

## Chrome MCP Verification

- Baseline:
  - `https://agenthub.hawkingrei.com/teams`
  - Old structure still showed the legacy sidebar with `Toggle team switcher`, `Teams console`, and no persistent `Teams N` index section.
- Post-edit regression:
  - `http://127.0.0.1:4173/teams`
  - Verified the new shell renders `TEAM SELECTOR`, scope tabs, and a persistent `Teams` section in both mobile and desktop viewports.
  - Local preview currently serves the built SPA without a matching API backend, so team data falls into the expected empty state (`No teams yet.`) and an API parse error banner may appear. This blocks populated-data visual verification, but the structural sidebar regression is still confirmed.
