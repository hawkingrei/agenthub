# cmd/agenthub Entrypoint Layout

## Summary

Moved the `agenthub` executable entrypoint sources into `cmd/agenthub/` without changing CLI or
server behavior.

## Why

The project now uses `cmd/agenthub/` as the physical home for the `agenthub` binary sources so the
CLI/main entrypoint layout is explicit and separated from the library-oriented `src/` tree, using a
TiKV-style `cmd/<binary>/Cargo.toml + src/` layout.

## What Changed

- moved `src/main.rs` to `cmd/agenthub/src/main.rs`
- moved `src/app.rs` to `cmd/agenthub/src/app.rs`
- moved `src/actor_cli.rs` to `cmd/agenthub/src/actor_cli.rs`
- added `cmd/agenthub/Cargo.toml` and registered `cmd/agenthub` as a workspace member so the
  binary entrypoint directory has a real `Cargo.toml + src/` layout without introducing a second
  lockfile or target directory
- kept the library wiring unchanged at the API level by path-including the moved modules from
  `src/lib.rs`
- kept the root `agenthub` package binary target, but rewired it to
  `cmd/agenthub/src/main.rs` so existing `cargo run -p agenthub --` and test helpers continue to
  work without behavior changes
- updated Bazel root targets so the library includes `cmd/agenthub/src/**/*.rs` and the binary
  target now uses `cmd/agenthub/src/main.rs`
- updated `AGENTS.md` to reflect the new thin-entrypoint location

## Validation

- `cargo test -p agenthub actor_cli::tests -- --nocapture`
- `cargo test -p agenthub app::tests -- --nocapture`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`
- `cargo check -p agenthub-cmd`
- `cargo clippy --locked -p agenthub-cmd --all-targets -- -D warnings`
- `bazel build //:agenthub`
- `cargo fmt --all --check`
- `git -c core.fsmonitor=false diff --check`
