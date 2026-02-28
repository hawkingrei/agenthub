# Agent Events BLOB + Zstd Storage

## Background

`agent_events` grew quickly due to high-volume ACP JSON payloads. Previous text-safe compression (`zstd + base64`) reduced size, but still paid base64 overhead and kept storage on a text path.

## Changes

- Switched event message persistence to a binary path:
  - `encode_message_for_storage` now returns `Vec<u8>`.
  - ACP rows store `prefix + zstd(payload)` directly as bytes.
  - non-ACP rows store raw UTF-8 bytes.
- Switched event reads to bytes:
  - `list_events` / `list_events_for_session` now read `message` as `Vec<u8>`.
  - Team memory flush observation decoding now reads `message` as `Vec<u8>`.
- Updated schema for new databases:
  - `agent_events.message` changed from `TEXT` to `BLOB`.
- Added startup migration for existing databases:
  - detect non-BLOB `agent_events.message`,
  - rebuild table with `BLOB` message column,
  - copy rows with `CAST(message AS BLOB)`,
  - preserve row IDs and data.
- Added regression tests for:
  - codec roundtrip and corruption fallback,
  - existing DB migration from TEXT to BLOB.

## Validation

- `cargo test --package agenthub event_message_codec`
- `cargo test --package agenthub init_db_migrates_agent_events_message_column_to_blob`
