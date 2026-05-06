# Team Message Projection Group ID Rollout

## Summary

Team actor mailbox messages and channel replica rows now carry nullable `group_id` metadata.
New actor messages inherit the owning run group, channel replicas copy the authority message group
or run group, and archive documents preserve the stored actor-message group.

## Background

The distributed metadata rollout already added nullable `group_id` storage to Team authority rows,
the node registry, and Team conversation messages. Actor mailbox rows are the canonical delivery
authority for coordinator/worker messages, while channel replica rows are node-relevant
projections. Both need the same group boundary before routing or projection enforcement can become
meaningful.

## Scope

- Add nullable `group_id` storage to `team_actor_messages`.
- Backfill existing actor message rows from their owning `team_runs.group_id`.
- Persist `group_id` for newly sent actor mailbox messages.
- Preserve `team_actor_messages.group_id` in actor-message archive/search documents.
- Add nullable `group_id` storage to `team_channel_message_replicas`.
- Backfill channel replicas from their authority message group or owning run group.
- Persist `group_id` for newly stored channel replica rows.
- Keep `group_id` internal and non-enforcing in this slice.

## Key Decisions

- `team_actor_messages.group_id` inherits from `team_runs.group_id`, not from node-local state.
- `team_channel_message_replicas.group_id` prefers the authority conversation message group and
  falls back to the run group for legacy or partial authority rows.
- Missing `group_id` remains `NULL` and means `unknown`.
- This slice does not enforce routing boundaries. Enforcement waits until projections and reviewed
  node group assignment are also in place.

## Validation

Recommended focused checks:

```bash
cargo test -p agenthub-db init_db_adds_and_backfills_team_actor_message_group_ids -- --nocapture
cargo test -p agenthub send_actor_message_persists_authority_group_id -- --nocapture
cargo test -p agenthub send_actor_message_dual_writes_created_rows_to_archive -- --nocapture
cargo test -p agenthub-db init_db_adds_and_backfills_channel_replica_group_ids -- --nocapture
cargo test -p agenthub internal_grpc_mailbox_send_persists_channel_replica_history -- --nocapture
cargo fmt --all --check
cargo check -p agenthub
```

## Follow-Ups

- Complete any remaining search projection checks around the newly populated message group values.
- Add a reviewed node group assignment source before enforcing cross-group routing.
