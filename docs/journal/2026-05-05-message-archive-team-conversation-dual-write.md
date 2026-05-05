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
- Archive append is best-effort after the SQLite insert succeeds. If archive append fails, the user
  visible conversation write remains committed and the failure is logged for later migration repair.
- The archive document uses `authority_message_id = message_id` and preserves `correlation_id` from
  the message payload when present.
- `body_text` is extracted from `payload.text`, then `payload.summary`, then the redacted JSON
  payload as a fallback.

## Validation

```bash
cargo fmt --all --check
cargo test -p agenthub-message-archive message_archive_backend_parses_config_values -- --nocapture
cargo test append_task_conversation_message_dual_writes_created_rows_to_archive -- --nocapture
cargo check
```

## Follow-Ups

- Add migration for historical `team_conversation_messages` rows into the archive.
- Extend live dual-write to Team run events and Team actor mailbox messages.
- Switch message search/read paths from direct SQLite scans to the archive search contract after
  migration coverage is in place.
