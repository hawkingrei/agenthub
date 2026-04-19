# Workspace Channel-First Lens Language

## Summary

Aligned the unified Workspace shell away from `Chat / Threads` language and back to a
channel-first information architecture.

## Decisions

- The top-level Workspace lens bar should use `Channels / Tasks / Members / Search`.
- `thread` is a channel-level secondary pane, not a top-level Workspace lens.
- Existing deep links that still use `?lens=chat` or `?lens=threads` should remain readable during
  rollout, but they should normalize to the canonical `channels` lens in the UI.
- Team continues to use `Channels / Kanban / Execution Runs / Members` as the primary object-local
  structure.

## Implementation Notes

- Updated shell route parsing to map legacy `chat` / `threads` lens values onto `channels`.
- Updated Team workspace header lens copy and helper tests to use `Channels`.
- Updated the unified Workspace IA spec and Team channels/threads spec to make `thread` explicitly
  subordinate to `channel`.

## Validation

- `cd web && npm run test -- vite.config.test.ts src/app_route_selection.test.ts src/pages/team_page.helpers.test.ts src/pages/team/team_page_header.test.tsx`
