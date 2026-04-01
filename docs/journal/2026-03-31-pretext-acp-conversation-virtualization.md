# ACP Conversation Pretext Virtualization

## Summary

Scoped `@chenglou/pretext` to ACP conversation virtualization as a text-aware height estimator for
`agent_message` and `user_message` rows. Keep tool/thinking/plan rows on the existing coarse
fallback path so the POC stays narrow and does not couple rendering to DOM measurement.

## Goals

- Reduce virtual spacer drift for long text-heavy ACP histories.
- Avoid per-scroll O(n) remeasurement by precomputing item heights and prefix sums.
- Keep the fallback path safe when canvas measurement is unavailable (for example under tests or
  restricted browser environments).

## Implementation

- Added `web/src/hooks/conversation_height_estimate.ts`:
  - wraps `@chenglou/pretext` `prepare`/`layout` behind message-only helpers
  - caches prepared text and estimated heights
  - builds prefix-sum height models for virtual slice calculation
- Updated `web/src/hooks/use_acp_conversation.ts`:
  - tracks conversation viewport width alongside scroll top/height
  - rebuilds the text-aware height model only when source items, width, or fallback average changes
  - keeps slice selection on binary search over prefix sums so scroll handling remains cheap
- Added focused unit coverage in `web/src/hooks/conversation_height_estimate.test.ts`.

## Validation

- `cd web && npx vitest run src/hooks/conversation_height_estimate.test.ts src/hooks/use_acp_conversation.test.ts src/hooks/use_acp_conversation.interaction.test.tsx src/acp_conversation_render.test.tsx`
- `cd web && npm run lint -- src/hooks/conversation_height_estimate.ts src/hooks/conversation_height_estimate.test.ts src/hooks/use_acp_conversation.ts`
- `cd web && npm run build`
- `git diff --check`

## Notes

- Chrome DevTools MCP validation was attempted before and after the change, but the MCP transport
  was unavailable in this environment (`Transport closed`). Browser-side regression still needs
  follow-up verification after deployment before closing the TODO item.
