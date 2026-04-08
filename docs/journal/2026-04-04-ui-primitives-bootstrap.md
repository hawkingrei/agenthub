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
  - `web/src/pages/team/team_selector_panel.tsx`
    - selector header and filter row now use `ToolbarRow`
    - create action now uses shared `ActionButton`
    - team rows now use `SelectableListItem`
  - `web/src/pages/team/team_debug_panels.tsx`
    - debug header and run forms now use `SurfaceCard`
    - debug tab switcher and run actions now use shared `ActionButton`
    - stacked input/action rows now use `ToolbarRow`
  - `web/src/pages/team/team_page_header.tsx`
    - selector shortcut now uses shared `ActionButton`
  - `web/src/pages/team/team_workspace_header.tsx`
    - agent workspace trigger and advanced workspace trigger now use shared `ActionButton`
  - `web/src/pages/team/team_management_modals.tsx`
    - modal footer actions now use shared `ActionButton`
    - repeated modal card shells now use `SurfaceCard`
  - `web/src/pages/team_sidebar.tsx`
    - sidebar section toggles now use shared `ActionButton`
    - team, workflow, and agent rows now use `SelectableListItem`
  - `web/src/pages/team_task_panel.tsx`
    - conversation shell now uses `SurfaceCard`
    - refresh row and composer meta row now use `ToolbarRow`
  - `web/src/pages/team_mailbox_panel.tsx`
    - mailbox shell now uses `SurfaceCard`
    - advanced controls shell now uses `InsetSurface`
    - chat composer action row now uses `ToolbarRow`
  - `web/src/pages/team_member_console_panel.tsx`
    - member detail shell now uses `InsetSurface`
  - `web/src/use_app_output_cache.ts`, `web/src/use_app_permissions.ts`, `web/src/use_app_sse_events.ts`, `web/src/use_app_acp_ui.ts`
    - cleaned up lingering type imports after the hook extraction so dispatcher/output-cache signatures match actual exports
  - `web/src/components/bubbles/markdown_bubble.tsx`, `web/src/pages/team_task_panel.tsx`, `web/src/pages/team_mailbox_panel.tsx`
    - removed duplicated base bubble class composition now that variant constants already include the shared bubble shell
  - `web/src/ui/primitives.tsx`
    - `ActionButton` and `SelectableListItem` now forward refs so Mantine `Menu.Target` can anchor shared button primitives safely
    - `SelectableListItem` now exposes an explicit `layout` prop so row/column direction does not rely on conflicting Tailwind utilities
  - `web/src/pages/team/team_selector_panel.tsx`
    - selector rows now use `layout="row"` instead of overriding the primitive's default column stack with conflicting flex classes
  - `web/src/use_app_agents.ts`, `web/src/app.tsx`
    - logout now clears pending node draft and agent selection state instead of leaving stale local state across sessions
    - target node selection now clamps back to `main` when the selected node disappears from refreshed runtime node inventory
    - agent workbench selection now falls back when the current agent disappears after refresh or deletion
  - `web/src/use_app_agents.test.tsx`
    - added regression coverage for logout state reset and invalid target node fallback after a node-list refresh

## Validation

- `cd web && npm run test -- src/ui/primitives.test.tsx src/agents_panel.test.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run build`
- `cd web && npm run test -- src/ui/primitives.test.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run lint`
- `make build-web`
- `cd web && npm run test -- src/pages/team/team_selector_panel.test.tsx src/pages/team/team_debug_panels.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm run test -- src/pages/team/team_page_header.test.tsx src/pages/team/team_workspace_header.test.tsx`
- `cd web && npm run test -- src/pages/team/team_management_modals.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm run test -- src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm run test -- src/use_app_agents.test.tsx src/app.route_shell.test.tsx src/app.runtime_effects.test.tsx`

## Follow-up

- Migrate remaining Team panels to the shared primitives instead of duplicating
  toolbar/button class strings.
- Extract shared list-row primitives once Agents row and Team sidebar/item affordances
  converge further.
