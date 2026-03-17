# Agents Sidebar Resize Persistence

## Summary

- added a desktop-only draggable splitter between the Agents sidebar and the output pane
- persisted the sidebar width in browser `localStorage` so the preferred width survives reloads
- kept mobile behavior unchanged by disabling the splitter below the desktop breakpoint
- switched narrow sidebar agent rows into a two-line layout so agent names stay visible while controls move to a second line
- reduced per-row control button size to fit narrow sidebar widths without collapsing the title area

## Files

- `web/src/app.tsx`
- `web/src/components/agents_panel.tsx`
- `web/src/styles.css`
- `web/src/app.permission_scope.test.ts`
- `web/src/agents_panel.test.tsx`

## Validation

- domain baseline checked on `https://agenthub.hawkingrei.com/` with Chrome DevTools MCP before edits:
  - no draggable splitter was present
  - mobile CSS still used the existing one-column / overlay behavior
  - narrow sidebar rows could lose the visible agent name because the action group stayed on the same line
- post-edit runtime validation on the domain is still pending until the web bundle is deployed

