# Message Archive Backend Completion Audit

## Summary

The LanceDB message archive backend rollout is complete for the current backend scope. The runtime now initializes the configured archive store, dual-writes new Team message-shaped records best-effort after SQLite commits, exposes Team-scoped archive search, and provides an admin migration route for legacy SQLite-resident message history.

## Background

The active P0+ backlog item required a backend-agnostic archive layer with LanceDB as the first backend, ACP chunk aggregation into logical messages, dual-write for new records, and one-way migration of existing SQLite message history without replacing Team relational state.

## Scope

- Backend archive abstraction and LanceDB implementation.
- Runtime archive initialization from app config.
- Team conversation message, actor mailbox message, run event, memory flush, and step lifecycle dual-write paths.
- Historical migration for Team conversation messages, Team run events, Team actor messages, main `agent_events`, and per-agent `AgentEventDbRouter` rows.
- Team-scoped search API backed by the archive abstraction.

## Key Decisions

- SQLite remains the transactional source for Team relational state.
- Archive writes are best-effort and bounded by a short timeout so message persistence is not dependent on LanceDB availability.
- Migration is one-way and idempotent through deterministic archive document IDs.
- Parseable ACP chunks are aggregated into `aggregated_acp_message` documents; malformed or non-chunk rows remain raw `agent_event` documents.
- Current live Team writes leave `group_id` empty until an authoritative group rollout exists, while the archive schema and filters preserve `group_id`.

## Validation

```bash
cargo test -p agenthub-message-archive
```

Result: 16 passed, 0 failed.

```bash
cargo test -p agenthub archive
```

Result: 17 passed, 0 failed, 611 filtered out.

The focused checks cover archive model/factory behavior, LanceDB append/search/upsert/schema migration, ACP chunk aggregation, Team conversation/actor/run-event dual-write, admin migration, Team search API scope enforcement, historical Team table migration, main and per-agent ACP event replay, and shared-thread mailbox exclusions.

## Follow-Ups

- Continue the separate distributed metadata ownership P0+ item for authoritative `group_id` rollout planning and broader human-visible metadata projection.
- Continue remaining Team/mobile/creation/adoption P0+ backlog items independently; they are not part of the message archive backend completion boundary.
