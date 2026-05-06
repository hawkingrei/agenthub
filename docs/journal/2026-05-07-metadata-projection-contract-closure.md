# Metadata Projection Contract Closure

## Summary

This checkpoint closes the remaining P0+ metadata ownership rollout across node registry
assignment, Team archive search, and run-event archive projections.

## Background

Earlier rollout slices added nullable `group_id` storage to Team authority rows, actor mailbox
rows, channel replicas, and message archive documents. The remaining gap was not a new routing
policy; it was making the current authority/projection contract explicit and mechanically usable
before later group enforcement.

## Scope

- `agent_nodes.group_id` is now writable through the root/admin node registry create and update
  paths on `main`.
- Team message archive search accepts a normalized `group_id` filter and passes it through to the
  configured archive backend.
- Team run-event archive documents inherit `group_id` from the owning `team_runs` row during live
  dual-write and migration.
- The canonical metadata and node-registry specs now describe the reviewed node assignment source
  and the remaining enforcement boundary.

## Key Decisions

- `main` remains the authority for node group assignment. Node-local mirrors and gossip
  observations must not invent or override `group_id`.
- Blank node `group_id` values normalize to `NULL`; non-empty assignments require a schema that can
  persist `agent_nodes.group_id`.
- Search uses `group_id` as an optional projection filter, not as an authorization boundary. Routing
  and tenant enforcement remain a later phase.
- Run events do not need their own authority `group_id` column because the owning run already
  carries the group boundary; archive documents project that boundary for search.

## Validation

Local focused checks:

```bash
cargo check -p agenthub
cargo test -p agenthub create_and_patch_agent_node_preserves_main_owned_group_id -- --nocapture
cargo test -p agenthub team_message_search_api_uses_archive_with_team_scope -- --nocapture
cargo test -p agenthub append_run_event_dual_writes_created_event_to_archive -- --nocapture
cargo test -p agenthub migrate_team_messages_to_archive_covers_team_message_tables -- --nocapture
cargo test -p agenthub-db init_db_adds_agent_nodes_group_id_column -- --nocapture
cargo fmt --all --check
```

## Follow-Ups

- Enforce cross-group mailbox, channel fan-out, and remote relay rejection only after the dedicated
  routing/gossip phase has a reviewed bridge policy.
- Add a dedicated groups table before treating current compatibility group ids as final tenant
  identities.
