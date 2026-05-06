# Team Message Group ID Rollout

## Summary

Team conversation messages now carry the nullable Team-derived `group_id` authority metadata.
New task conversation messages persist the owning task group, and archive documents copy that value
when dual-writing or migrating Team conversation message history.

## Background

The distributed metadata contract makes `group_id` the future multi-tenant and routing isolation
boundary. Earlier slices added nullable `group_id` storage to Team authority rows and the node
registry. This slice moves the same ownership metadata into the first human-visible message authority
table without exposing it as a public API contract.

## Scope

- Add nullable `group_id` storage to `team_conversation_messages`.
- Backfill existing message rows from their owning `team_tasks.group_id`.
- Persist `group_id` for newly appended task conversation messages and shared-thread chat replies.
- Preserve the message `group_id` in Team conversation archive/search documents.
- Keep `group_id` skipped from serialized `TeamConversationMessageRecord` responses for now.

## Key Decisions

- `team_conversation_messages.group_id` inherits from `team_tasks.group_id`, not directly from a
  node-local value.
- Missing `group_id` remains `NULL` and means `unknown`; it is not treated as cross-group access.
- This slice does not enforce routing boundaries. Enforcement waits until message authority,
  projection, and node registry surfaces all carry compatible group metadata.

## Validation

Recommended focused checks:

```bash
cargo test -p agenthub-db init_db_adds_and_backfills_team_conversation_message_group_ids -- --nocapture
cargo test -p agenthub append_task_conversation_message_persists_authority_group_id -- --nocapture
cargo test -p agenthub append_task_conversation_message_dual_writes_created_rows_to_archive -- --nocapture
cargo fmt --all --check
cargo check -p agenthub
```

## Follow-Ups

- Propagate `group_id` into `team_actor_messages` authority rows.
- Copy `group_id` into channel replica and other searchable projection rows.
- Add a reviewed node group assignment source before enforcing cross-group routing.
