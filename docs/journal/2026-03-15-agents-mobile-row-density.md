# Agents Mobile Row Density

## Summary

- Compressed the mobile `Agents` list so row content focuses on agent selection and quick actions.
- On small screens, the list now shows the agent name plus action buttons, while model badge, status badge, workdir, and code-mode detail stay in the detail header/body.
- Reduced mobile action-button size to keep the left rail usable on narrow widths.

## Files

- `web/src/components/agents_panel.tsx`
- `web/src/agents_panel.test.tsx`

## Validation

- Local validation target:
  - `cd web && npx vitest run src/agents_panel.test.tsx src/output_header.test.tsx src/workbench_mode_switch.test.tsx src/pages/team_panels.test.tsx`
  - `cd web && npx eslint src/components/agents_panel.tsx src/agents_panel.test.tsx`
  - `cd web && npx vite build`
- Chrome DevTools MCP:
  - Mobile viewport `390x844` baseline showed the expanded Agents rail still included dense row content.
  - Post-edit regression should confirm the list remains name-first on mobile while detail information stays visible in the output header/body.
