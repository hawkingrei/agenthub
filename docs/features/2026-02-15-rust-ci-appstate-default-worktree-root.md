# Rust CI AppState Default Worktree Root

## Summary

Fix Rust CI compile failure in `api/agents` tests by updating the local
`AppState` test initializer to include the new `default_worktree_root` field.

## Background

`AppState` gained a required `default_worktree_root: String` field. The
`src/api/agents.rs` test helper still initialized `AppState` with the old
field list, causing `E0063` (missing field) during Rust CI.

## Scope

- `src/api/agents.rs`
- `docs/todo.md`

## Key Decisions

1. Keep the fix minimal and local to the failing test helper.
2. Reuse runtime config default via `config.default_worktree_root()` to keep
   test setup consistent with other modules (for example `api/teams` tests).

## Validation

```bash
cargo test -p agenthub api::agents::tests --no-run
cargo test -p agenthub --no-run
```
