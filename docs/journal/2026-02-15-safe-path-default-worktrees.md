# Safe Path Default `~/.agenthub/worktrees`

## Summary

Ensure `~/.agenthub/worktrees` is always treated as a safe path at runtime,
even when `safe_paths` is not explicitly configured in `config.toml`.

## Background

AgentHub defaults create-worktree paths under `~/.agenthub/worktrees`. Without
a matching safe-path baseline, a fresh config may fail safe-path checks for the
default worktree location.

## Scope

- `src/config.rs`
- `src/state.rs`
- `docs/todo.md`

## Key Decisions

1. `AppConfig::safe_paths()` now always includes `~/.agenthub/worktrees`.
2. Configured safe paths are still accepted, but blank entries are ignored and
   duplicates are removed while preserving insertion order.
3. `AppState::seed_safe_paths()` keeps using `config.safe_paths()`, so DB seed
   behavior automatically inherits this default baseline.

## Validation

```bash
cargo test safe_paths_ -- --nocapture
cargo test seed_safe_paths_ -- --nocapture
```
