# Conversation Stickiness Hysteresis

## Summary

Prevent accidental loss of auto-stick-to-bottom when conversation content or viewport changes without intentional user upward scrolling.

## Background

The previous stick-state update relied only on `isNearBottom(...)` distance checks.
During live content growth or viewport transitions, the scroll container could become "not near bottom" transiently even when the user did not scroll up, which flipped `stickToBottom` to `false` and stopped auto-follow.

## Scope

- `web/src/hooks/use_acp_conversation.ts`
- `web/src/hooks/use_acp_conversation.test.ts`
- `docs/todo.md`

## Key Decisions

1. Add `deriveConversationStickToBottom(...)` helper with stick-state hysteresis.
2. Keep sticky mode when:
   - currently sticky, and
   - bottom-distance check fails, but
   - no meaningful upward user movement is detected.
3. Introduce an upward movement epsilon (`24px`) so tiny pointer jitter does not detach stick mode.
4. Continue using near-bottom detection to re-enter sticky mode when user scrolls back to bottom.

## Validation

```bash
npm --prefix web run test -- hooks/use_acp_conversation.test.ts
npm --prefix web run build
```

Expected outcomes:

- New stickiness helper tests pass.
- Helper coverage confirms sticky mode survives passive growth / tiny jitter and detaches after meaningful upward scroll.
- Web build succeeds.

## Follow-ups

- Add browser-level interaction coverage for "typing with live updates" to verify stick behavior under real viewport and input focus changes.
