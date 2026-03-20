# Team Channel Rendering Performance

## Summary

- changed `Teams -> all` so the channel timeline only renders markdown for the visible tail window instead of materializing the entire conversation first
- changed Team conversation refresh to preserve unchanged `TeamConversationMessageRecord` object identity across SSE/poll refreshes
- kept the existing tail-window behavior (`latest 10` by default, expand on jump-to-top) intact

## Why

The previous channel pipeline still did expensive work for the full message history:

1. sort all messages
2. render markdown for every message
3. only then apply `windowConversation(..., 10)`

That meant long shared threads still paid the CPU and memory cost of the full history even though the UI only showed the latest tail window.

The refresh path also replaced the full `taskMessages` array on every refresh, which caused avoidable object churn and larger React diffs.

## Implementation

- `web/src/pages/team_task_panel.tsx`
  - sort once into `orderedMessages`
  - apply `windowConversation` before markdown rendering
  - render markdown only for `visibleWaterfallItems`
  - use `orderedMessages.length` for tail/jump thresholds and empty-state checks

- `web/src/pages/team/page_helpers.ts`
  - add `mergeConversationMessages(...)`
  - preserve unchanged message objects by `message_id` when refresh payloads are semantically unchanged
  - return the previous array instance when the refresh result is identical

- `web/src/pages/team_page.tsx`
  - use `mergeConversationMessages` inside `refreshTaskMessages`

- `web/src/pages/team_conversation_panel.tsx`
  - move Team channel `seenBy` derivation and panel prop assembly out of `TeamPage`
  - keep the conversation subtree behind a memoized boundary so unrelated TeamPage state churn is less likely to recalculate channel-specific derived data

- `web/src/pages/team_panels.test.tsx`
  - add a regression test proving hidden tail-window messages do not trigger markdown rendering until history is expanded

- `web/src/pages/team/page_helpers.test.ts`
  - add regression tests for conversation merge identity preservation

- `web/src/pages/team_page.smoke.test.tsx`
  - keep a minimal render-path smoke test around the Team page while the conversation boundary keeps moving

## Validation

- `cd web && npx vitest run src/pages/team/page_helpers.test.ts src/pages/team_panels.test.tsx`
- `cd web && npm run lint -- src/pages/team/page_helpers.ts src/pages/team/page_helpers.test.ts src/pages/team_task_panel.tsx src/pages/team_panels.test.tsx src/pages/team_page.tsx`
- `cd web && npm run build`
- `git -c core.fsmonitor=false diff --check`

## Follow-up

- the next higher-ROI step is to reduce TeamPage rerender scope so conversation updates do not pull the whole workbench through the same render path
- the next higher-ROI step is to continue moving conversation-owned state and callbacks out of `TeamPage` so `Runs`, `Kanban`, and agent views stay cold during channel-only updates
