# 2026-04-05 Design System Tokenization And Primitives

## Summary

- moved the most repeated Notion-style surface colors and shadows into `web/tailwind.config.cjs`
- replaced several remaining raw Team workflow buttons with Mantine-backed shared primitives
- aligned floating surfaces and Team tabs/composer shells to the new semantic Tailwind tokens
- corrected the Team retention TODO note from `recent-10` to the current `recent-20` behavior

## Implementation Notes

- Added semantic Notion tokens for:
  - subtle backgrounds
  - elevated/overlay surfaces
  - faint/subtle borders
  - subtle hover fills
  - user bubble tint
- Added shared box-shadow tokens for:
  - floating panels
  - soft cards
  - compact rows
  - tabs
  - dock surfaces
  - top inset border lines
- Switched `SurfaceCard` to Mantine `Box`.
- Switched `ActionButton` to Mantine `UnstyledButton`.
- Switched `IconButton` to Mantine `ActionIcon` with `unstyled`.
- Switched `TeamTabsBar` to Mantine `UnstyledButton`.
- Switched `TeamStepsPanel` refresh/submit/apply controls to shared `ActionButton`.
- Switched `TeamTasksPanel` task cards to `SelectableListItem`, toolbar buttons to `ActionButton`, and section headers to `ToolbarRow`.
- Switched `TeamSidebar` high-frequency controls from raw buttons to Mantine-backed `IconButton`, `ActionButton`, or `UnstyledButton`.
- Switched `TeamMailboxPanel` toolbar, accept, send, and advanced raw-mailbox actions to shared `ActionButton`/`ToolbarRow`.
- Switched `AcpDebug` tabs to Mantine `UnstyledButton`, session-control actions to shared `ActionButton`, and mode/model/config inputs to Mantine `NativeSelect`/`TextInput`.
- Aligned ACP Debug tests to the shared Mantine jsdom helper so primitive-backed controls keep a stable test harness.

## Validation

- run focused web tests around primitives and Team panels
- run `npm run lint`
- run `npm run build`
- run `make build-web`
- verify Team surface regressions in Chrome DevTools MCP on the live site after deploy
