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

## Follow-up Progress

- Extended `web/src/ui/primitives.tsx` with:
  - `InsetSurface`
  - `ToolbarRow`
  - `SelectableListItem`
- Backed the new list-row primitive with Mantine `UnstyledButton` so Team list affordances
  start converging on the shared focus and interaction path instead of raw `button` tags.
- Migrated more Team surfaces onto the shared primitives:
  - `web/src/pages/team_overview_panel.tsx`
    - playbook surface now uses `InsetSurface`
    - member mailbox rows now use `SelectableListItem`
  - `web/src/pages/team_run_panel.tsx`
    - run browser body now uses `InsetSurface`
    - list header/footer and run actions use `ToolbarRow`
    - run rows now use `SelectableListItem`
  - `web/src/pages/team_active_run_panel.tsx`
    - run action bar now uses `ToolbarRow`
    - refresh/cancel/resume/restart now use shared `ActionButton`
  - `web/src/pages/team_mailbox_panel.tsx`
    - actor selection rows now use `SelectableListItem`

## Validation

- `cd web && npm run test -- src/ui/primitives.test.tsx src/agents_panel.test.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run build`
- `cd web && npm run test -- src/ui/primitives.test.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run lint`
- `make build-web`

## Follow-up

- Migrate remaining Team panels to the shared primitives instead of duplicating
  toolbar/button class strings.
- Extract shared list-row primitives once Agents row and Team sidebar/item affordances
  converge further.
