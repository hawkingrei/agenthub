# Message Body Store Phase 1 Dual Write

**Date:** 2026-07-13

## Summary

Completed the opt-in Phase 1 conversation-message body rollout. New messages retain their complete
`payload_json` in SQLite and stage the same serialized body to `message_body_outbox` in the authority
transaction. The asynchronous drainer writes the staged copy to RocksDB `cf_body` and deletes the
outbox row only after acknowledgement.

## Compatibility And Migration

- Added `message_body_backfill_checkpoint` to record the historical message-id prefix staged to the
  outbox. Backfill is resumable and never replaces a SQLite body with a sentinel.
- Restored support for rows produced by the earlier sentinel implementation: migration reads the body
  from `cf_body` or the outbox before restoring `payload_json`; missing bodies leave the sentinel row
  unchanged and fail the pass for a later retry.
- Made the body store opt-in through `message_body_store.enabled = true`. Disabling it keeps the
  SQLite-only path intact.

## Validation

- Focused conversation tests cover dual writes, outbox retry, checkpointed idempotent backfill,
  legacy-sentinel restoration, store outage/corruption isolation for Phase 1 reads, and concurrent
  append/drain/read behavior.
- `cargo fmt --check` was run after the changes.

## Follow-Up

The remaining P1 storage task is the body-free `cf_index` delivery projection, its repair path, and
operational backup validation. SQLite bodies remain in place until a separately reviewed Phase 2.
