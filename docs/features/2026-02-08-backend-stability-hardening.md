# Backend Stability Hardening

## Background

Agent sessions produce high-frequency event writes and rely on in-memory handles for lifecycle state. Without explicit single-instance semantics and SQLite tuning, we risk orphaned processes, growing pending maps, and intermittent "database is locked" failures.

## Scope

- Enforce single running session per `agent_id`.
- Remove ACP permission pending entries on timeout.
- Enable SQLite WAL, set `synchronous=NORMAL`, and configure `busy_timeout`.
- Add composite index for session-scoped event queries.
- Clamp event list limits to protect the database.

## Key Decisions

- Treat each `agent_id` as a single-instance runtime: concurrent starts are rejected.
- Track in-flight starts to prevent race conditions before the runtime handle is registered.
- Use WAL + busy timeout to reduce write contention without changing the data model.
- Add a composite `(agent_id, session_id, seq)` index to keep pagination queries stable.

## Validation

- Manual: attempt to start the same agent twice and confirm the second start is rejected.
- Manual: trigger a permission request, wait past timeout, and confirm pending map does not grow.
- Manual: run a long-lived agent and confirm event pagination remains responsive.
- Automated:

```bash
cargo test -p agenthub
```

## Follow-ups

- Decide whether multi-session per `agent_id` is required and design a runtime model if so.
