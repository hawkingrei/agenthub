# Workbench Mode Switch Unification

## Summary

- Added a shared top-level `WorkbenchModeSwitch` component for `Agents` and `Teams`.
- Replaced the old single-icon cross-link in the Agents header and the back-link in the Teams header with the shared switch.
- Removed the duplicate `Agents / Teams` switch from the expanded Agents sidebar so route switching now has one primary location.
- Moved agent list collapse/expand ownership fully back to the left rail by removing the duplicate toggle from the right-side output header.
- Reduced mobile header weight on the Agents page: narrower switch, smaller top-shell padding, smaller action controls, and hidden subtitle on small screens.
- Hid the large `AgentHub` brand block on mobile and collapsed agent detail meta/subtitle on small screens so conversation keeps more first-screen height.
- Tightened header density while landing the shared navigation: smaller titles, lighter shadows, and slimmer action controls.

## Files

- `web/src/components/workbench_mode_switch.tsx`
- `web/src/app.tsx`
- `web/src/pages/team_page.tsx`
- `web/src/components/agents_panel.tsx`
- `web/src/components/output_header.tsx`
- `web/src/agents_panel.test.tsx`
- `web/src/output_header.test.tsx`
- `web/src/workbench_mode_switch.test.tsx`

## Validation

- Local validation target:
  - `cd web && npx eslint src/app.tsx src/components/agents_panel.tsx src/components/workbench_mode_switch.tsx src/pages/team_page.tsx src/agents_panel.test.tsx src/workbench_mode_switch.test.tsx`
  - `cd web && npx vitest run src/agents_panel.test.tsx src/output_header.test.tsx src/workbench_mode_switch.test.tsx src/pages/team_panels.test.tsx`
  - `cd web && npx vite build`
- Chrome DevTools MCP baseline:
  - Confirmed production `https://agenthub.hawkingrei.com/` still exposes a single `Teams` top-link and `https://agenthub.hawkingrei.com/teams` uses a separate `Agents` return link before this change.
- Chrome DevTools MCP regression:
  - Mobile regression should confirm only the left rail owns `Show agents` / `Hide agents`, the top `AgentHub` shell no longer dominates vertical space at narrow widths, and agent detail meta is collapsed on small screens.
