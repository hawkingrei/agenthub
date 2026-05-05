# Message Archive Team Conversation Dual Write

## Summary

Team conversation message creation now dual-writes newly created SQLite rows into the configured
message archive. This is the first live message-shaped write path connected to the archive trait.

## Background

The LanceDB archive phase 1 introduced the backend-agnostic archive crate, LanceDB implementation,
search contract, and ACP chunk aggregation. The next rollout step is to start feeding new
message-shaped records into the archive without replacing the relational SQLite tables.

## Scope

- Open the configured message archive during application service initialization.
- Pass the archive trait object into `TeamManager`.
- Append a deterministic `team_conversation_message:<conversation_id>:<message_id>` document when a
  Team conversation message row is newly created.
- Keep idempotent retries from writing duplicate archive documents.

## Key Decisions

- SQLite remains the transactional system of record for Team conversation messages.
- Archive append is best-effort after the SQLite insert succeeds. The append is dispatched in the
  background with a bounded timeout, so archive latency does not block the user-visible
  conversation write. Failures are logged for later migration repair.
- Archive initialization is best-effort for this rollout slice. Startup logs failures and disables
  dual-writes instead of making the message archive a hard boot dependency.
- The archive document uses `authority_message_id = message_id` and preserves `correlation_id` from
  the message payload when present.
- `body_text` is extracted from `payload.text`, then `payload.summary`. If neither field carries
  text, it stays empty because the full structured payload is already preserved in `payload_json`.

## Validation

```bash
cargo fmt --all --check
cargo test -p agenthub-message-archive message_archive_backend_parses_config_values -- --nocapture
cargo test --lib append_task_conversation_message_dual_writes_created_rows_to_archive -- --nocapture
cargo test --lib append_task_conversation_message_does_not_wait_for_slow_archive -- --nocapture
cargo test --lib message_archive_body_text_does_not_index_structured_payload_fallback -- --nocapture
cargo check
```

## Follow-Ups

- Add migration for historical `team_conversation_messages` rows into the archive.
- Extend live dual-write to Team run events and Team actor mailbox messages.
- Switch message search/read paths from direct SQLite scans to the archive search contract after
  migration coverage is in place.
