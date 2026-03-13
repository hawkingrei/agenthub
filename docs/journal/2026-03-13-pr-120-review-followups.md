# PR 120 Review Follow-ups

## Summary

Addressed remaining review feedback for the safe-path normalization and team runtime repair PR.

## Changes

- Replaced substring-based runtime start error classification with `TeamRuntimeStartError` downcasting in `src/api/teams/errors.rs`.
- Made `adjust_worker_runtime_workdir_for_safe_paths` fail explicitly when neither the derived worktree root nor the configured workdir is inside allowed safe paths.
- Moved test-only actor binary path probing behind `#[cfg(test)]` in `src/acp/runtime.rs` so production uses `current_exe()` directly.
- Introduced a crate-local `src/path_utils.rs::expand_tilde` helper and reused it from `config`, `api/admin`, `agent/manager/codec`, `db`, and `team/runtime` to avoid further divergence.
- Added a clarifying comment in `src/agent/manager/runtime.rs` that `worktree_ref` is a Git ref rather than a filesystem path, so tilde expansion does not apply.

## Validation

- `cargo test -p agenthub map_runtime_start_error_maps_typed_runtime_config_errors_to_bad_request -- --nocapture`
- `cargo test -p agenthub expand_tilde_uses_home_join_for_relative_paths -- --nocapture`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`
