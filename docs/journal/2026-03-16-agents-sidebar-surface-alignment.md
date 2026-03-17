# Agents sidebar surface alignment

## Summary

The Agents sidebar used a much darker surface palette than the rest of the workbench, especially once the refreshed row classes stopped inheriting legacy white-card styles.

## Problem

- The sidebar container used a near-black surface.
- Expanded agent rows used a separate dark slate surface.
- Collapsed `Agents` / `Running` metric tiles also stayed dark.

That stack made the left rail feel like a different product from the rest of the light workbench shell.

## Change

- Moved the Agents sidebar container to the same light warm surface family as the output workbench.
- Changed expanded agent rows to warm ivory cards with a softer green-tinted active state.
- Changed collapsed rail metric tiles (`Agents`, `Running`) from dark slate to muted warm cards.
- Kept action buttons and status controls distinct enough to stay scannable without turning the whole rail into a dark block.

## Validation

- `cd web && npx vitest run src/agents_panel.test.tsx`
- `cd web && npm run lint -- src/components/agents_panel.tsx src/agents_panel.test.tsx src/ui/tailwind_classes.ts`
- `cd web && npm run build`

## Chrome MCP baseline

- Confirmed on `https://agenthub.hawkingrei.com/` that the current sidebar still mixes legacy light surfaces with stronger dark controls, and the collapsed metrics feel visually detached from the rest of the shell.
- Post-change domain verification still needs deployment because this turn only updated local code.
