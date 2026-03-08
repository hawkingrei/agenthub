# 2026-03-06 Teams Slock Layout Direction Reset

## Context

We attempted a broader UI shell refresh that touched auth, Agents, shared shell structure,
and Teams at the same time. That scope was too wide for the immediate goal and increased
mobile layout risk.

The working tree was fully reset afterwards. No part of that abandoned UI batch is intended
to be preserved.

## Decisions

1. Restart from a clean tree.
- All uncommitted UI changes from the abandoned broad shell attempt were discarded.
- Follow-up work should start from the current clean `main`-based state.

2. Limit the next UI task to `/teams`.
- Do not change `Agents`.
- Do not change auth/login pages.
- Keep runtime behavior and Team feature flow intact; change layout/presentation only.

3. Use Slock as a layout reference, not a skin reference.
- Copy layout grammar, hierarchy, and density where helpful.
- Do not copy Slock color palette or decorative style.
- The target is a stronger Team workbench composition, not a visual clone.

4. Keep the current project coding style.
- Reuse existing React component boundaries.
- Reuse current Tailwind/Mantine patterns and Team class-token approach.
- Avoid introducing a parallel styling system or broad global CSS rewrites.

5. Treat mobile safety as a first-class constraint.
- Avoid aggressive global height/overflow locking.
- Avoid fixed-position mobile shells unless strictly necessary and verified.
- Prefer incremental Team-local layout changes over app-wide shell rewrites.

## Known Constraint

Chrome DevTools MCP is currently unavailable in this environment:

- `chrome-devtools/list_pages` -> `Transport closed`
- `chrome-devtools/new_page` -> `Transport closed`

The next Team UI change should still include Chrome DevTools MCP checks once transport is
restored, because frontend verification policy still requires MCP-based before/after review.

## Next Scope

The next implementation pass should focus on:

- `web/src/pages/team_page.tsx`
- `web/src/pages/team_sidebar.tsx`
- Team-local Tailwind tokens and component-local layout ordering as needed

The intended direction is:

- clearer left-rail + main-workbench composition;
- stronger title/status/action hierarchy;
- tighter panel rhythm and denser content ordering;
- mobile ordering that preserves primary task visibility.

## Result

This journal records the reset and the new task boundary before implementation resumes.
