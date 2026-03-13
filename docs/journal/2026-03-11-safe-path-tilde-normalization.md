## Summary

Normalize default and configured `safe_paths` to absolute paths before storing them in SQLite, and migrate legacy `~/.agenthub/worktrees` rows on startup.

## Why

The runtime comparison path already expands `~`, but the default seed path was persisted as the literal string `~/.agenthub/worktrees` while admin mutations stored expanded absolute paths. That created inconsistent storage-level representations for the same allowlist entry.

## Changes

- `AppConfig::safe_paths()` now returns expanded absolute paths.
- `AppState::seed_safe_paths()` persists normalized values via the config boundary.
- `db::init_db_at_path()` now runs a one-time `safe_paths` normalization migration:
  - insert expanded absolute path if missing
  - delete legacy tilde row

## Validation

- `cargo test init_db_normalizes_safe_paths_to_absolute_paths -- --nocapture`
- `cargo test seed_safe_paths_inserts_default_when_not_configured -- --nocapture`
- `cargo test safe_paths_includes_default_worktrees_path -- --nocapture`
