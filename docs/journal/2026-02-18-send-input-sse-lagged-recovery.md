# Send Input SSE Lagged Recovery

## Background

Users reported intermittent "no response after send input" behavior in web sessions.
Two gaps could produce this symptom:

1. Frontend polling was suppressed whenever SSE was connected, even during the post-send boost window.
2. Backend SSE forwarder ignored `broadcast::RecvError::Lagged`, so a connection could stay open while dropping events.

## Scope

- Keep existing SSE + polling architecture.
- Improve recovery when live SSE delivery misses events.
- Avoid unbounded client/server retry loops.

## Key Decisions

1. Frontend polling policy:
   - Poll when SSE is disconnected, as before.
   - Also poll during `boostUntil` after user sends input, even if SSE is connected.
2. Frontend SSE stale watchdog:
   - Track last SSE activity timestamp from `onopen/onmessage`.
   - If SSE reports `OPEN` but no activity for a stale window, force reconnect and fallback polling.
3. Backend lagged handling:
   - Treat broadcast lag as stream degradation.
   - Close the SSE stream so client reconnect + DB replay path can recover missing events.

## Implementation

- `web/src/event_polling.ts`
  - Add `shouldPollAgentEvents(...)` and `isSseConnectionStale(...)`.
- `web/src/event_polling.test.ts`
  - Add coverage for connected/disconnected + boost-window + stale-SSE behavior.
- `web/src/app.tsx`
  - Use `shouldPollAgentEvents(...)` in poll loop.
  - Clear expired boost marker and allow poll during boost even with SSE open.
  - Add stale-SSE watchdog and proactive reconnect.
- `src/sse.rs`
  - Change `RecvError::Lagged` handling from `continue` to `disconnect + break`.
  - Add warning log for lagged close.
- `src/sse.rs` tests
  - Add `output_stream_closes_after_broadcast_lagged`.

## Validation

Executed (2026-02-19):

```bash
npm --prefix web run test -- src/event_polling.test.ts src/app.permission_scope.test.ts
cargo test -p agenthub sse::tests::output_stream_closes_after_broadcast_lagged -- --nocapture
```

## Follow-up

- Real browser verification is still required for long-running sessions with high output throughput and rapid input/send cycles.
