# Node Registry Group ID

## Summary

The canonical `agent_nodes` registry now has a nullable `group_id` column. This gives the
main-owned node registry the same forward-compatible group boundary storage that Team authority rows
received in the previous slice.

## Background

The distributed node registry contract requires `group_id` to become live on node authority rows
before gossip scoping or cross-group routing enforcement can be added. Node-local mirrors must not
invent group membership, so the first step is a nullable authority column owned by `main`.

## Scope

- Added nullable `agent_nodes.group_id` storage to new SQLite schemas.
- Added an idempotent migration for existing `agent_nodes` tables.
- Preserved existing rows with `group_id = NULL`; this phase does not infer a group for nodes.

## Key Decisions

- This slice does not expose `group_id` through the Agent Node HTTP API.
- Existing node rows remain unresolved for group routing until a later explicit group assignment
  path lands.
- Gossip scoping and mailbox/remote relay enforcement remain deferred.

## Validation

```bash
cargo fmt --all --check
cargo test -p agenthub-db init_db_adds_agent_nodes_group_id_column -- --nocapture
```

## Follow-Ups

- Add a reviewed group assignment source for node registry rows.
- Mirror `group_id` into node-local registry snapshots as read-only authority metadata.
- Enforce cross-group routing only after message authority rows and node registry rows both carry
  populated group boundaries.
