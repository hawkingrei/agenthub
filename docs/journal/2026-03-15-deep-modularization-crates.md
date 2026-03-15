# Journal: Deep Modularization of Core Services

- **Date:** 2026-03-15
- **Task:** Extract logging, config, and db into standalone workspace crates.

## Summary

As a follow-up to the initial `AppState::init` refactor and as outlined in the `AGENTS.md` project constraints, the monolithic parts of the application logic in `src/` have been broken out into independent domain libraries. This brings us fully into alignment with the Bazel-oriented dependency structure and improves isolation.

## Changes

1. **`agenthub-logging`**: Extracted complex log rotation and tracing initialization logic (`ActiveHourlyLogWriter`, `init_tracing`) from `src/app.rs`.
2. **`agenthub-config`**: Extracted application configuration (`src/config.rs`) and shared path utilities (`src/path_utils.rs`). The config is foundational and needed by almost every other crate.
3. **`agenthub-db`**: Extracted SQLite initialization, pragmas, migration logic, and basic routing (`src/db.rs`).

## Test Improvements

During test verification, it was found that the external `http_proxy` configurations locally caused the `TeamRemoteRelayAdapter`'s `reqwest::Client` to fail during mock server tests (due to attempting to route `127.0.0.1` traffic through a proxy). Added a `.no_proxy()` override to the default builder and added a stable `100ms` yield time in the `axum` test server spawn loop to prevent timing flakes under heavy concurrent load (`cargo test --all`).

## Verification

- Built using Bazel (`BUILD.bazel` targets configured for all new crates).
- Passed full cargo test suite `cargo test --all` reliably across multiple workers.
- Passed full `cargo clippy --workspace --all-targets -- -D warnings`.
