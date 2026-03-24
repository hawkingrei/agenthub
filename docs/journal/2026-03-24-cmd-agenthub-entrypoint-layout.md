# cmd/agenthub Entrypoint Layout

## Summary

Moved the `agenthub` binary bootstrap under `cmd/agenthub/` while keeping shared CLI/server
implementation in the root library tree.

## Why

The project now uses `cmd/agenthub/` as the physical home for the executable bootstrap so the
binary entrypoint is explicit and separated from the library-oriented `src/` tree, closer to a
TiKV-style `cmd/<binary>/Cargo.toml + src/` layout without mixing `cmd/` sources into the
library module tree.

## What Changed

- moved `src/main.rs` to `cmd/agenthub/src/main.rs`
- added `cmd/agenthub/Cargo.toml` so the binary entrypoint directory has a real `Cargo.toml + src/`
  layout
- added an empty `[workspace]` to `cmd/agenthub/Cargo.toml` so the standalone manifest can be
  checked directly without rejoining the root workspace and recreating the duplicate `agenthub`
  binary ambiguity
- kept shared CLI/server implementation in `src/app.rs` and `src/actor_cli.rs` as normal library
  modules instead of path-including files from `cmd/`
- kept the root `agenthub` package binary target, rewired to
  `cmd/agenthub/src/main.rs` so existing `cargo run -p agenthub --` and test helpers continue to
  work without behavior changes
- kept `cmd/agenthub` outside the root workspace members to avoid introducing a second workspace
  binary target named `agenthub`
- updated Bazel root targets so the binary target now uses `cmd/agenthub/src/main.rs` while the
  library remains sourced from `src/**/*.rs`
- updated `AGENTS.md` to reflect the new thin-entrypoint location

## Validation

- `cargo test -p agenthub actor_cli::tests -- --nocapture`
- `cargo test -p agenthub app::tests -- --nocapture`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`
- `cargo check --manifest-path cmd/agenthub/Cargo.toml --offline`
- `cargo clippy --manifest-path cmd/agenthub/Cargo.toml --all-targets --offline -- -D warnings`
- `cargo fmt --all --check`
- `git -c core.fsmonitor=false diff --check`

## Notes

- `cmd/agenthub` now owns its own ignored `Cargo.lock` and `target/` when checked directly as a
  standalone manifest. The root workspace keeps the canonical tracked `Cargo.lock`.
- Local `bazel build //:agenthub` is currently blocked by an existing `rules_rust` package-loading
  problem in this environment (`@@rules_rust+//rust:defs.bzl` missing package metadata), so Bazel
  validation is left to CI for this change.
