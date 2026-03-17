# Agents sidebar name visibility fix

## Summary

The Agents left rail could lose the visible agent name even though the record was present and selectable.

## Root cause

The refreshed workbench row styling reused legacy global class names such as `.agent-row`.
Legacy CSS in `web/src/styles.css` still forced a white row background, while the new Tailwind row classes kept white text.
That combination made the left-rail agent name effectively invisible on the deployed page.

## Change

- Renamed the refreshed Agents workbench row classes to `agents-workbench-*` so they no longer collide with legacy global selectors.
- Kept the row visuals owned by the Tailwind utility classes in `web/src/ui/tailwind_classes.ts`.
- Preserved only structural layout rules in `web/src/styles.css` for the new scoped selectors.
- Updated focused `AgentsPanel` tests to cover the new scoped class names.

## Validation

- `cd web && npx vitest run src/agents_panel.test.tsx`
- `cd web && npm run lint -- src/components/agents_panel.tsx src/agents_panel.test.tsx src/ui/tailwind_classes.ts`
- `cd web && npm run build`

## Chrome MCP baseline

- Confirmed on `https://agenthub.hawkingrei.com/` that the left rail card lost the visible agent name while the main header still showed the selected agent name.
- Confirmed the left rail row screenshot still showed model/workdir/status controls, which ruled out missing data and pointed to a styling collision.
