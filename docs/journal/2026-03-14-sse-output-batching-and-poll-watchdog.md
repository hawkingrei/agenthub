# SSE Output Batching And Poll Watchdog

## Background

Live Codex ACP sessions could keep writing events into the per-agent event DB
while the browser appeared to stop rendering new output. Runtime logs showed
two related symptoms:

- bursty SSE delivery could hit bounded fan-in backpressure and close the
  stream for replay recovery;
- frontend output recovery was coupled too tightly to the SSE subscription
  lifecycle, so a broken subscription could also stop `/events` catch-up for
  the active agent.

## Scope

- Batch live SSE transport payloads in `src/sse.rs` without changing persisted
  `agent_events` granularity.
- Teach the web client to consume batched SSE payloads.
- Move active-agent `/events` catch-up polling into an independent watchdog in
  `web/src/app.tsx`.
- Add focused regression coverage for the new SSE payload shape.

## Key Decisions

- Keep the database and replay path unchanged: each `AgentOutput` is still
  persisted individually with the same `event_id/session_id/seq` semantics.
- Batch only at the SSE transport boundary:
  - flush when the batch reaches 32 events;
  - flush when estimated batch size reaches 64 KiB;
  - flush every 50 ms to cap added latency.
- Preserve the legacy SSE message shape for single-event payloads and emit a
  dedicated `{ type: "batch", payload: AgentOutput[] }` message only when the
  transport actually coalesces multiple events.
- Keep active-agent polling independent from SSE open/close state so replay
  catch-up remains available even when SSE is reconnecting or stale.

## Validation

```bash
cargo test sse::tests::output_stream_batches_multiple_messages_into_single_sse_event
cd web
./node_modules/.bin/vitest run src/app.permission_scope.test.ts
./node_modules/.bin/eslint src/app.tsx src/app.permission_scope.test.ts
./node_modules/.bin/vite build
```

Expected outcomes:

- high-frequency ACP chunk streams emit fewer SSE messages without changing
  persisted event ordering;
- frontend accepts both legacy single-event SSE payloads and new batched
  payloads;
- active-agent `/events` replay continues to run even after SSE reconnects or
  temporary stream stalls.
