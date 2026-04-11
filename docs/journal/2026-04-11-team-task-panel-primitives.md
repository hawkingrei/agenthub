# Team Task Panel Primitive Consolidation

## Summary

Continued the Team frontend primitive rollout on `web/src/pages/team_task_panel.tsx`
so the conversation surface stops carrying its own bespoke empty-state and metadata
shells.

## Changed

- `web/src/ui/primitives.tsx`
  - added `EmptyState`
  - added `KeyValueList`
  - added `KeyValueItem`
- `web/src/pages/team_task_panel.tsx`
  - moved the seen-progress hovercard into a focused local component
  - moved developer-mode message details into a focused local component backed by
    shared key/value primitives
  - switched channel loading/empty feedback to the shared empty-state primitive
- `web/src/ui/primitives.test.tsx`
  - added SSR coverage for the new empty-state and key/value primitives
- `web/src/pages/team_panels.test.tsx`
  - added regression coverage for the expanded message-details panel
  - added regression coverage for the empty channel state

## Validation

- Chrome DevTools MCP baseline attempt before edits: blocked because the local
  `chrome-devtools` transport closed before a page session could be established
- `cd web && npm run test -- src/ui/primitives.test.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run lint -- src/ui/primitives.tsx src/ui/primitives.test.tsx src/pages/team_task_panel.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run build`
