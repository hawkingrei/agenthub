# Mobile Agents Toggle Header Relocation

## Summary

- moved the mobile agents collapse/expand control into the top workbench header
- removed the temporary narrow-screen collapsed rail so the conversation area stays clean when the sidebar is hidden
- kept mobile sidebar toggling on a single path instead of splitting controls between the left rail and content area

## Details

- added a mobile-only header button in `web/src/app.tsx` that toggles the agents sidebar open and closed
- restored narrow-screen `.workspace-left.collapsed` behavior to hide the sidebar completely when collapsed
- hid the in-panel collapse button on mobile so the header owns the only explicit collapse affordance
- kept collapsed rail render assertions in `web/src/agents_panel.test.tsx` for desktop/tablet collapsed layouts

## Validation

- baseline inspection used Chrome DevTools MCP against `https://agenthub.hawkingrei.com/` at `390x844` and confirmed the current mobile header has room for a dedicated sidebar toggle
- local regression should verify that mobile users can reopen the agents sidebar from the top header while the collapsed sidebar stays fully hidden off-canvas
