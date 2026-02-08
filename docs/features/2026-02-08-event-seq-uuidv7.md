# Event Sequence UUIDv7

## Background

Event ordering and pagination relied on numeric `seq` values. To avoid JavaScript precision limits and standardize ordering across API, SSE, and polling, the sequence identifier is now a UUIDv7 string.

## Scope

- Change `agent_events.seq` from numeric to `TEXT` and write UUIDv7 values.
- Update APIs to accept `before_seq` as a string and return `seq` as a string.
- Update frontend ordering, cursor, and cache logic to compare UUIDv7 strings.

## Key Decisions

- Use UUIDv7 for time-ordered sequences with lexicographic sorting.
- Treat string comparison as the canonical ordering rule for `seq`.
- Skip backfill/migration because there are no existing users or data.

## Validation

- Manual: start an agent, confirm event ordering via UI and `before_seq` pagination.
- Automated:

```bash
cargo test -p agenthub -- tests/web_assets.rs
cd web && npm test
```

## Follow-ups

- Revisit migration strategy if persistent data becomes a requirement.
