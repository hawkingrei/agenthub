# UI Primitives Bootstrap

## Summary

Added a thin internal UI primitive layer on top of Mantine + Tailwind so repeated
surface/header/button patterns stop drifting independently across Agents and Team pages.

## Added

- `web/src/ui/primitives.tsx`
  - `cx(...)`
  - `SurfaceCard`
  - `PanelHeader`
  - `ActionButton`
  - `IconButton`
  - `StatusPill`

## Applied

- `web/src/components/agents_panel.tsx`
  - toolbar actions
  - rail actions
  - row icon actions
  - row tags / pills
- `web/src/pages/team_overview_panel.tsx`
  - panel shell
  - header action
  - leader id pill
- `web/src/pages/team_run_panel.tsx`
  - panel shell
  - header actions
  - primary / secondary / danger actions
  - team id pill

## Validation

- `cd web && npm run test -- src/ui/primitives.test.tsx src/agents_panel.test.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run build`

## Follow-up

- Migrate remaining Team panels to the shared primitives instead of duplicating
  toolbar/button class strings.
- Extract shared list-row primitives once Agents row and Team sidebar/item affordances
  converge further.
