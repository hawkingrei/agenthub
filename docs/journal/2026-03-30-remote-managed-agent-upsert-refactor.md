# 2026-03-30 Remote Managed Agent Upsert Refactor

## Summary

- refactored remote managed agent persistence so `ensure_remote_managed_agent` no longer duplicates legacy-schema-aware `INSERT` / `UPDATE` field assembly
- kept runtime behavior unchanged:
  - remote managed records still normalize `target_node_id` to `NULL`
  - remote managed insert still defaults status to `created`
  - remote managed upsert still leaves `agent_loop_*` fields untouched

## Why

The previous `upsert_remote_managed_agent_record(...)` path built two separate SQL shapes:

- one `UPDATE agents SET ...`
- one `INSERT INTO agents (...) VALUES (...)`

Both repeated the same schema-compat branching for:

- `target_node_id`
- `source`
- shared remote-managed fields such as `name`, `workdir`, `command`, `args`, `worktree_*`, and `code_mode`

That made the remote-managed path harder to review and easier to drift from the intended legacy-schema contract.

## What Changed

- introduced `RemoteManagedAgentPersisted` as the shared projection for remote-managed rows
- centralized:
  - insert column assembly
  - insert value assembly
  - update assignment assembly
- kept `insert_agent_record(...)` unchanged because the remote-managed path intentionally persists a narrower field set than regular local-agent creation

## Focused Validation

- `cargo test -p agenthub remote_managed_upsert_ -- --nocapture`
- `cargo clippy -p agenthub --all-targets -- -D warnings`
- `git diff --check`

## Regression Coverage

- full-schema insert keeps:
  - `target_node_id = NULL`
  - `source`
  - `status = created`
  - remote-managed field projection
- legacy-schema update keeps:
  - existing `status`
  - existing `agent_loop_*`
  - updated remote-managed fields without requiring `source` / `target_node_id`
