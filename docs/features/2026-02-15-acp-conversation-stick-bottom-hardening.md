# ACP Conversation Stick-to-Bottom Hardening

## Background

Conversation auto-stick occasionally failed to return to the latest bottom position
when large batches of ACP events arrived in a short time window.

At the same time, tail-key generation for tool call updates used `JSON.stringify`
on payloads, which increased per-update CPU cost for large nested payloads.

## Scope

- Harden auto-stick behavior for bursty conversation updates.
- Reduce per-update overhead in conversation tail-key generation.
- Keep frozen view, jump button, and load-older behavior unchanged.

## Key Decisions

- Replace duplicated bottom-alignment effects with a single scheduled alignment path.
- Add RAF-throttled bottom alignment plus strict near-bottom retry passes
  (up to two extra frames) to handle delayed layout growth during heavy updates.
- Limit viewport sync-on-tail updates to virtualization mode only.
- Replace `JSON.stringify` payload length probing in `buildConversationTailKey`
  with bounded structural size estimation (`estimateTailPayloadSize`) to avoid
  large serialization cost on every tail update.

## Validation

```bash
cd web
npm run test -- src/hooks/use_acp_conversation.test.ts src/acp_conversation.test.ts src/acp_conversation_render.test.tsx
npm run build
```

- Expect helper and render suites to pass.
- Expect build to pass without TypeScript regressions.

## Follow-up

- Validate on a real long-running session with frequent tool call updates
  (search/explore/shell mixed streams) to confirm no intermittent stick-bottom drift.
