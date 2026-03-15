# Journal: Deep Modularization of Core Services

- **Date:** 2026-03-15
- **Task:** Extract logging, config, and db into standalone workspace crates.

## Summary

As a follow-up to the initial `AppState::init` refactor and as outlined in the `AGENTS.md` project constraints, the monolithic parts of the application logic in `src/` have been broken out into independent domain libraries. This brings us fully into alignment with the Bazel-oriented dependency structure and improves isolation.

## Changes

1. **`agenthub-logging`**: Extracted complex log rotation and tracing initialization logic (`ActiveHourlyLogWriter`, `init_tracing`) from `src/app.rs`.
2. **`agenthub-config`**: Extracted application configuration (`src/config.rs`) and shared path utilities (`src/path_utils.rs`). The original files in `src/` have been removed, and `src/lib.rs` now re-exports `agenthub_config` as `config` and `agenthub_config::path_utils` as `path_utils`.
3. **`agenthub-db`**: Extracted SQLite initialization, pragmas, migration logic, and basic routing (`src/db.rs`). The original file in `src/` has been removed, and `src/lib.rs` now re-exports `agenthub_db` as `db`.

## Test Improvements

During test verification, it was found that the external `http_proxy` configurations locally caused the `TeamRemoteRelayAdapter`'s `reqwest::Client` to fail during mock server tests (due to attempting to route `127.0.0.1` traffic through a proxy). Added a conditional `.no_proxy()` override for tests. Improved the relay test server startup by replacing the hard-coded sleep with a `/health` readiness probe (polling every 5ms for up to 50 attempts, with explicit timeouts).

## Implementation Details

- **Concurrency**: Fixed a potential race condition in `AgentEventDbRouter::pool_for_agent` by using `tokio::sync::OnceCell` to ensure only one database pool is initialized per agent ID.
- **Robustness**: Updated `expand_tilde` to fallback to `.` if the `HOME` environment variable is not set.
- **Test Stability**: Added `connect_timeout` and `timeout` to the test readiness probe client to prevent tests from hanging on failure.

## Verification

- Built using Bazel (`BUILD.bazel` targets configured for all new crates).
- Passed full cargo test suite `cargo test --all` reliably across multiple workers.
- Passed full `cargo clippy --workspace --all-targets -- -D warnings`.
