# ACP Conversation Blank Window Guard

## Summary

Fix intermittent ACP conversation blank-window behavior where users could see an
empty conversation area until clicking `Jump to bottom`.

## Background

A recurring UI issue was reported:

- conversation area occasionally became visually empty ("white screen"),
- visible content returned only after pressing the down-arrow jump control.

Two risk points were identified in the conversation scroll pipeline:

1. virtual slice start index could overshoot valid bounds for large/stale
   `viewportTop` values, producing an empty rendered window;
2. stale freeze state across agent/session transitions could leave
   `frozenItems` empty while actual conversation messages already existed.

## Scope

- `web/src/hooks/use_acp_conversation.ts`
- `web/src/hooks/use_acp_conversation.test.ts`
- `docs/todo.md`

## Key Decisions

1. Harden virtual list slicing with explicit viewport/index clamping:
   - clamp `viewportTop` to estimated scrollable range,
   - clamp `start` to `maxStart`,
   - ensure `end >= start + 1` when list is non-empty.
2. Reset freeze/stick state on active agent/session switch:
   - default back to stick-to-bottom mode,
   - clear stale freeze cursor/items/pending counters.
3. Add an in-hook self-heal guard:
   - when frozen mode is active, messages exist, but frozen view is empty,
     rebuild freeze baseline from current messages;
   - if rebuilding still yields empty view, jump to bottom as safety fallback.

## Validation

```bash
cd web
npm run test -- src/hooks/use_acp_conversation.test.ts
npm run lint -- src/hooks/use_acp_conversation.ts src/hooks/use_acp_conversation.test.ts
npm run build
```

## Follow-ups

- Add browser-level regression (Playwright) that simulates stale large
  `scrollTop` + session switch and asserts non-empty conversation rendering.
