# 2026-03-19 Team Channel Stick-To-Bottom

## Goal

Align `Teams -> all` scrolling with the standalone agent conversation mental model:

- default to the latest messages
- keep following the bottom while the user has not scrolled away
- stop auto-follow once the user scrolls upward
- provide an explicit jump-back action

## Changes

- Added local `stickToBottom` state to `TeamTaskPanel`.
- Added bottom-alignment on initial render and when a new tail message arrives while the view is
  still pinned to the bottom.
- Added scroll detection so manual upward scrolling disables auto-follow.
- Added quick navigation actions above the composer:
  - `Jump to top` when the thread opens at the latest messages and the backlog is long enough
  - `Jump to bottom` whenever the shared thread is no longer pinned to the latest messages

## Validation

- `cd web && npx vitest run src/pages/team_panels.test.tsx`
- `cd web && npm run lint -- src/pages/team_task_panel.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run build`

## Notes

- This intentionally mirrors the agent conversation behavior at the interaction level, without
  pulling in the ACP virtualization stack.
- Live browser verification should use a logged-in visible browser session rather than the removed
  Chrome MCP flow.
