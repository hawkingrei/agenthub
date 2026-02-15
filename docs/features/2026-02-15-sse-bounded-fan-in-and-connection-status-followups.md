# SSE Bounded Fan-In And Connection Status Follow-Ups

## Background

Review feedback highlighted two stability risks in the SSE path:

- backend fan-in used `mpsc::unbounded_channel`, which can grow without bound for
  slow/disconnected clients under bursty multi-agent output;
- frontend status/error handling had duplicated constants and a few edge-case
  correctness gaps (auth-error detection order, stale offline banner, and
  unnecessary ACP cache work on non-ACP lines).

## Scope

- Replace SSE fan-in queue with a bounded channel in `src/sse.rs`.
- Keep live stream bounded and rely on persisted DB history + polling/reconnect
  for catch-up semantics.
- Align frontend connection/error constants usage and remove duplicated literals.
- Improve frontend SSE error handling and connection badge accessibility.
- Reduce avoidable ACP cache updates for non-ACP stream lines.
- Expand unit tests for connection status helpers.

## Key Decisions

- Use `tokio::sync::mpsc::channel(512)` instead of `unbounded_channel` for
  output fan-in.
- Forwarder tasks use timeout-guarded async send:
  - if queue pressure clears quickly, continue streaming;
  - if queue stays full past timeout (`2s`), mark stream as backpressured and
    close the SSE stream.
- On stream close, frontend reconnect + polling/DB replay fills any missed
  events instead of keeping unbounded in-memory buffers.
- Keep `broadcast::RecvError::Lagged` behavior as-is; missed live events are
  acceptable because events are persisted and can be replayed.
- Export shared connectivity constants from `web/src/connection_status.ts` and
  reuse them from `web/src/app.tsx`.
- Check `isInvalidTokenMessage` against raw SSE payload before sanitization.
- Update ACP cache only for `stream === "acp"` entries.

## Validation

```bash
cargo test sse::tests
cd web
npm run test -- connection_status.test.ts
npm run build
```

Expected outcomes:

- SSE output fan-in remains bounded under slow clients;
- sustained queue pressure closes the current SSE stream and recovers through
  reconnect + persisted history replay;
- frontend connection badge/error banner states are consistent;
- auth redirect detection remains correct on SSE error payloads;
- connection-status helper coverage includes connecting/empty/whitespace and
  false-positive guard cases.
