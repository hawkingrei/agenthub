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
- Switched `TeamTaskPanel` permission actions, refresh action, mention picker options, seen-state affordances, details toggle, jump-to-bottom control, and send button onto Mantine-backed `ActionButton`, `IconButton`, or `UnstyledButton`.
- Switched `WorkbenchHeaderMenu` trigger onto Mantine `UnstyledButton` so the last header menu trigger no longer depends on a raw HTML button.
- Added semantic Tailwind tokens for ACP code/payload/plan surfaces and moved the most visible `plan_bubble` plus ACP terminal/payload card colors off inline hex usage.
- Aligned ACP Debug tests to the shared Mantine jsdom helper so primitive-backed controls keep a stable test harness.
- Wrapped `TeamSidebar`, `TeamMailboxPanel`, `TeamMemberAcpPanel`, `TeamTabsBar`, and `TeamStepsPanel` in `React.memo` to stop unrelated Team workbench state from re-running the heaviest sidebar/mailbox/ACP render trees.
- Wrapped `TeamRunPanel`, `TeamEventsPanel`, `TeamOverviewPanel`, `TeamMemberConsolePanel`, and `TeamActiveRunPanel` in `React.memo` so toolbar/runtime/detail state changes no longer fan out across inactive Team surfaces.
- Added a focused `TeamMemberAcpPanel` regression test that verifies unrelated parent state changes no longer rebuild ACP view state.
- Wrapped `AcpDebug` in `React.memo` so parent chrome/output state churn no longer re-renders the ACP debug terminal/session/runtime subtree unless debug props actually change.
- Stabilized `team_page.tsx` callback props that were defeating memo boundaries (`onSelectTeam`, `onRefreshSteps`, mailbox template handlers), so the new Team panel memoization actually short-circuits parent workbench rerenders.
- Extracted ACP progressive disclosure controls into `web/src/components/acp_progressive_views.tsx` so the show-more footer and segmented window hooks no longer live inline inside `acp_tool_content.tsx`.
- Wrapped the heaviest ACP tool renderers (`ToolPayloadView`, `ToolCallDetailsView`, `ToolTextContent`, `TerminalOutputView`) in `React.memo` to stop unrelated conversation state churn from re-running payload formatting on every render.
- Switched `acp_request_user_input_cards.tsx` submit actions onto shared `ActionButton` and memoized `RequestUserInputCard` to keep inline approval/question cards aligned with the shared primitive layer.
- Switched `admin_page.tsx`, `join_page.tsx`, and `output_error_boundary.tsx` off their remaining raw buttons and onto Mantine-backed shared primitives.
- Moved the Team create-agent accent control from hard-coded hex colors to semantic `brand-primary`/`brand-primary-hover` tokens.
- Updated ACP conversation / join / error-boundary tests to use the shared Mantine jsdom helpers so `matchMedia` and related provider assumptions stay consistent across the suite.
- Migrated the highest-traffic raw buttons in `team_page.tsx` onto shared primitives or Mantine `UnstyledButton`, including the header sidebar toggle, runtime-notice dismiss action, Team selector refresh, selector rows, agent workspace menu trigger, advanced-workspace menu trigger, debug tab chips, and the repeated `Go to Runs` empty-state CTAs.
- Switched `agent_node_section.tsx` machine-picker cards onto `SelectableListItem`, leaving the shared-primitives migration with only app-shell/test-helper raw buttons outside the new primitive layer.

## Validation

- run focused web tests around primitives and Team panels
- run `npm run lint`
- run `npm run build`
- run `make build-web`
- verify Team surface regressions in Chrome DevTools MCP on the live site after deploy
- verify the live Team task surface keeps `Refresh channel`, `Pending delivery`, `Show details`, and the composer button interactive after the Mantine primitive swap
- verify the live workbench menu trigger still opens the dropdown after the trigger swap to Mantine `UnstyledButton`
- run `npm run test -- src/acp_conversation_render.test.tsx src/acp_conversation.interaction.test.tsx src/create_agent_modal.test.tsx src/create_agent_modal.interaction.test.tsx src/pages/admin_page.test.tsx src/pages/join_page.test.tsx src/output_error_boundary.test.tsx`
- run `npm run lint -- --ignore-pattern dist-debug --ignore-pattern dist-debug-current`
- run `npm run build`
- run `make build-web`
- verify on `https://agenthub.hawkingrei.com/teams/276a2682-9ce7-4af5-aa6c-f12575d13c37` that `Refresh channel`, `Pending delivery`, `Show details`, and `Send` remain interactive after the primitive migration
- verify on `https://agenthub.hawkingrei.com/` that the workbench menu trigger still opens and no new console errors appear beyond the known `favicon.ico 404`
- run `npm run test -- src/pages/team_page.smoke.test.tsx src/pages/team_panels.test.tsx src/workbench_header_menu.test.tsx src/workbench_header_menu.interaction.test.tsx`
