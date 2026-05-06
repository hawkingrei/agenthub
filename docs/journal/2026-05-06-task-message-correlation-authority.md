# Task Message Correlation Authority

## Summary

`team_conversation_messages` now persists `correlation_id` as a first-class authority column instead
of relying only on `payload_json.correlation_id`.

## Background

The logical message metadata contract treats canonical human-visible conversation messages as
authority rows on `main`. Those rows already carried `conversation_id` as a physical column and
used the row id as `authority_message_id`, but `correlation_id` was still only embedded in
`payload_json`. That made authority-row reconciliation depend on payload conventions.

## Scope

- Add `team_conversation_messages.correlation_id` to the fresh SQLite schema.
- Add an idempotent migration for existing databases.
- Backfill existing rows from `payload_json.correlation_id` when present.
- Persist the correlation id for new task conversation messages and human-visible chat replies.
- Keep message archive/search behavior unchanged; archive documents already derive the same
  metadata from the canonical message payload.

## Key Decisions

- The column is `TEXT NOT NULL DEFAULT ''` to keep legacy rows and direct manager call paths
  backward-compatible while still making the authority reference queryable.
- Backfill is best-effort and logs a warning instead of blocking startup if a legacy payload shape
  cannot be read.
- This slice does not add `group_id` to authority rows. `group_id` remains a separate rollout step
  because it needs a live authority ownership decision for multi-tenant boundaries.

## Validation

```bash
cargo test -p agenthub-db init_db_adds_task_message_correlation_id_and_backfills_existing_rows -- --nocapture
cargo test -p agenthub append_task_conversation_message_persists_correlation_id_column -- --nocapture
cargo fmt --all --check
```

## Follow-Ups

- Continue the distributed metadata rollout for `group_id` authority ownership.
- Audit any remaining human-visible or searchable projection that still depends only on payload
  metadata for `authority_message_id` or `correlation_id`.
