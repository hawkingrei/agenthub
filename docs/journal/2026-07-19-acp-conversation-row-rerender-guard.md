# ACP Conversation Row Rerender Guard

## Summary

Reduced avoidable ACP conversation row rerenders by memoizing the row wrapper around
`AcpConversationBubble`. Focus changes now rerender only rows whose focused state changes, while
unrelated visible rows keep their existing rendered bubble and row wrapper state.

## Background

ACP conversations already had virtualization, progressive rendering, and memoized bubble rendering.
However, the row wrapper around each bubble still rerendered for every visible item when
`focusedToolCallId` changed, even when the changed focus target did not affect that row. Long
ACP-heavy windows can have many visible rows, so the focus/jump path should avoid work outside the
affected tool-call rows.

## Scope

This slice covers:

- `AcpConversationItemRow` memoization
- a comparator that treats focused state as row-local
- focused unit coverage for unrelated focus changes and render-affecting prop changes

It does not change virtualization thresholds, Team channel rendering, or backend event fetch
behavior.

## Key Decisions

- Row identity remains based on the existing conversation item key and item props.
- `focusedToolCallId` is compared by whether it affects the current row, not by raw string equality.
- Render-affecting props such as row item, global index, latest visible index, run status, markdown
  render version, ANSI renderer, and submit callback still force a row update.
- Existing `AcpConversationBubble` memoization remains the inner rendering guard.

## Validation

```bash
cd web && npm exec vitest -- run src/acp_conversation.test.ts src/acp_conversation_render.test.tsx src/acp_conversation.interaction.test.tsx
git diff --check
```

## Follow-Ups

- Continue the broader frontend performance TODO with Team long-list surfaces and cross-page
  rerender audits.
- Evaluate whether Team conversation rows need the same row-local focus/update guard after the ACP
  slice settles.
