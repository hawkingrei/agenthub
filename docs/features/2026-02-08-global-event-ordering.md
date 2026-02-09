# Global Event Ordering

## Background

Agent output ordering relied on UUIDv7 `seq` values. This provides approximate ordering but cannot guarantee a strict global sequence across concurrent writers and multiple agents. The A2A requirement calls for a globally consistent event order that is replayable and stable across API, SSE, and polling.

## Scope

- Treat `agent_events.id` (SQLite autoincrement) as the authoritative global order.
- Expose `event_id` on API, SSE, and WS payloads.
- Add `before_id` pagination and drop `before_seq` from the API surface.
- Update frontend ordering, cursors, and cache logic to prefer `event_id`.

## Key Decisions

- Use database insert order as the global sequence authority.
- Persist first, then emit, to guarantee every emitted event has a stable `event_id`.
- Keep `seq` for ACP semantics and diagnostics, but do not use it for ordering or pagination.
- Ensure ordering helpers treat missing `event_id` and `ts` deterministically to avoid unstable UI sorting.

## Validation

- Manual: run multiple agents concurrently and confirm UI ordering stays stable across refresh, SSE, and polling.
- Automated:

```bash
cargo test -p agenthub -- tests/web_assets.rs
cd web && npm test
```

## Follow-ups

- Decide whether to backfill `event_id` into existing caches or to invalidate on upgrade.
- Confirm any external integrations that rely on `before_seq` are migrated to `before_id`.
