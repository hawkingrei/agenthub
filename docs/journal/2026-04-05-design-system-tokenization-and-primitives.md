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
- Switched `AcpPanel` tab chrome and jump-to-bottom affordance to Mantine `UnstyledButton`.
- Switched the ACP Debug permission history jump affordance off the last remaining raw `button`.
- Switched `InputDock` jump/interrupt/history/send controls to Mantine `UnstyledButton` while preserving the existing Tailwind affordance classes and overlay layout.
- Aligned ACP Debug tests to the shared Mantine jsdom helper so primitive-backed controls keep a stable test harness.
- Wrapped `TeamSidebar`, `TeamMailboxPanel`, `TeamMemberAcpPanel`, `TeamTabsBar`, and `TeamStepsPanel` in `React.memo` to stop unrelated Team workbench state from re-running the heaviest sidebar/mailbox/ACP render trees.
- Wrapped `TeamRunPanel`, `TeamEventsPanel`, `TeamOverviewPanel`, `TeamMemberConsolePanel`, and `TeamActiveRunPanel` in `React.memo` so toolbar/runtime/detail state changes no longer fan out across inactive Team surfaces.
- Added a focused `TeamMemberAcpPanel` regression test that verifies unrelated parent state changes no longer rebuild ACP view state.
- Wrapped `AcpDebug` in `React.memo` so parent chrome/output state churn no longer re-renders the ACP debug terminal/session/runtime subtree unless debug props actually change.
- Stabilized `team_page.tsx` callback props that were defeating memo boundaries (`onSelectTeam`, `onRefreshSteps`, mailbox template handlers), so the new Team panel memoization actually short-circuits parent workbench rerenders.

## Validation

- run focused web tests around primitives and Team panels
- run `npm run lint`
- run `npm run build`
- run `make build-web`
- verify Team surface regressions in Chrome DevTools MCP on the live site after deploy
