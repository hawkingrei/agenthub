# Message Archive Team Migration

## Summary

Added a Team SQLite history migration path into the message archive and made LanceDB archive writes
idempotent by `document_id`.

## Background

The previous message archive slices introduced the backend-agnostic archive contract, LanceDB as the
first backend, Team conversation dual-write, and the first Team-scoped archive search API. The
remaining gap was historical Team data that already lived only in SQLite.

## Scope

- Add `TeamManager::migrate_team_messages_to_archive(batch_size)` for historical Team message rows.
- Convert `team_conversation_messages`, `team_run_events`, and `team_actor_messages` into canonical
  archive documents.
- Add a root-only admin trigger at `POST /api/admin/message_archive/team_messages/migrate`.
- Process migration source rows in bounded batches and append each batch immediately.
- Preserve stable document identities for re-runnable migration:
  - `team_conversation_message:<conversation_id>:<message_id>`
  - `team_run_event:<run_id>:<event_id>`
  - `team_actor_message:<run_id>:<message_id>`
- Upsert LanceDB archive batches by `document_id` so migration retries replace the same logical
  document instead of appending duplicates.

## Key Decisions

- Keep the Team migration entrypoint on `TeamManager` for this slice because it already owns the
  SQLite Team schema and archive trait object.
- Put idempotent upsert behavior in the LanceDB backend adapter rather than making every migration
  caller reason about backend-specific duplicate handling.
- Use `run_event.event_type` as searchable body fallback when a run-event payload has no human text.
- Resolve run-event `task_id` and `conversation_id` from the run input when the event payload lacks
  those fields.
- Keep actor mailbox migration scoped to the target actor in `agent_id`, preserve
  `authority_message_id` from channel fan-out payloads, and recognize `channel_conversation_id`.
- Exclude `shared_thread_mailbox` bootstrap runs from run-event and actor-mailbox migration so
  internal transport bookkeeping does not appear in Team message search.

## Validation

```bash
cargo test -p agenthub-message-archive lancedb_archive_upserts_documents_by_document_id -- --nocapture
cargo test --lib migrate_team_messages_to_archive_covers_team_message_tables -- --nocapture
cargo clippy --locked -p agenthub --lib -- -D warnings
cargo fmt --all --check
git diff --check
```

## Follow-Ups

- Add historical ACP event aggregation into archive documents.
- Extend live dual-write beyond Team conversation messages where the write path can tolerate
  best-effort archive append semantics.
