# Channel Replica Correlation Projection

## Summary

This slice promotes `correlation_id` from a payload-only convention into a first-class `team_channel_message_replicas` projection column.

## Background

The logical message metadata contract requires channel replica rows to preserve `run_id`, `conversation_id`, `authority_message_id`, and `correlation_id` so node-local caches remain reconcilable against `main` authority rows. Before this slice, internal gRPC channel replica ingestion validated `correlation_id`, but the replica table only retained it inside `payload_json`.

## Scope

- Add `team_channel_message_replicas.correlation_id` to the fresh SQLite schema.
- Add legacy migration/backfill from `payload_json.correlation_id`.
- Persist the validated internal gRPC channel replica `correlation_id` when storing replica rows.
- Update focused tests to assert the physical projection column.

## Key Decisions

- `team_channel_message_replicas` remains a replica/cache table, not an authority table.
- The migration uses an empty-string default only to make SQLite legacy-column addition safe; rows with payload-level `correlation_id` are backfilled immediately during `init_db`.
- Internal gRPC still rejects channel replica payloads without `correlation_id`, so new replica rows should carry a non-empty first-class value.

## Validation

```bash
cargo test -p agenthub-db init_db_adds_channel_replica_correlation_id_and_backfills_existing_rows -- --nocapture
cargo test -p agenthub internal_grpc_mailbox_send_persists_channel_replica_history -- --nocapture
cargo test -p agenthub internal_grpc_mailbox_send_rejects_channel_replica_payload_without_correlation_id -- --nocapture
cargo fmt --all --check
```

## Follow-Ups

- Continue the broader distributed metadata P0+ rollout by deciding when `group_id` becomes an authority-layer field instead of projection compatibility metadata.
- Continue auditing other human-visible/searchable projections for payload-only metadata that should become first-class columns.
