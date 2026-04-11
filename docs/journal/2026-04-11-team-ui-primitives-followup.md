# Team UI Primitives Follow-up

## Summary

Continued the Team workbench primitive consolidation by migrating two remaining Team panels onto the shared Mantine + Tailwind primitive layer.

## Scope

- `web/src/ui/primitives.tsx`
- `web/src/ui/primitives.test.tsx`
- `web/src/pages/team_member_status_strip.tsx`
- `web/src/pages/team_member_status_strip.test.tsx`
- `web/src/pages/team_steps_panel.tsx`

## Changes

1. Extended the shared primitive layer with:
   - `EmptyState`
   - `InlineNotice`
   - `KeyValueList`
   - `KeyValueItem`
2. Migrated `TeamMemberStatusStrip` to shared shells and metadata layout:
   - use `SurfaceCard` for the outer panel shell
   - use `ToolbarRow` for the title + summary header
   - use `InsetSurface` for member cards
   - use `StatusPill` for lifecycle summary counters
   - use `KeyValueList` / `KeyValueItem` for role/agent/current metadata
   - use `EmptyState` for the empty-member path
3. Migrated `TeamStepsPanel` to the same primitive path:
   - use `SurfaceCard` for the outer panel shell
   - use `InsetSurface` for submit/action sections and step rows
   - use `InlineNotice` for the list-only developer-mode hint
   - use `EmptyState` when no steps are available
   - use `KeyValueList` / `KeyValueItem` for step metadata rows

## Validation

- `cd web && npm run test -- src/ui/primitives.test.tsx src/pages/team_member_status_strip.test.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run lint -- src/ui/primitives.tsx src/pages/team_member_status_strip.tsx src/pages/team_steps_panel.tsx src/ui/primitives.test.tsx src/pages/team_member_status_strip.test.tsx`
- `cd web && npm run build`

## Chrome DevTools MCP

- Attempted local baseline and post-change regression checks against the worktree Vite dev server.
- The first attempt was blocked by a stale `chrome-devtools-mcp` Chrome profile already in use.
- After clearing the stale Chrome processes, the DevTools MCP transport closed before a new page session could be created.
- This turn therefore includes source/test/build verification, but no completed MCP snapshot evidence.
