# Summary

Keep `agenthub` CLI failures readable even when `RUST_BACKTRACE=1` is present in
the environment.

# Why

Team and ACP runtimes now commonly set `RUST_BACKTRACE=1` so agent subprocesses
produce actionable panic diagnostics. The binary entrypoint still relied on
Rust's default `main() -> Result` error reporting, which prints a full backtrace
for ordinary user-facing CLI validation failures as well. Commands such as
`agenthub actor permission-review-respond ...` therefore emitted long stack
traces for simple issues like missing `team_id` runtime context.

# What Changed

- Added a small CLI error renderer that prints a compact `Error:` line plus an
  optional cause chain.
- Switched `cmd/agenthub/src/main.rs` from returning `anyhow::Result<()>` to
  returning `ExitCode`, with explicit error reporting through the new helper.
- Added a focused integration test that runs the real `agenthub` binary with
  `RUST_BACKTRACE=1` and verifies actor CLI validation failures no longer emit a
  `Stack backtrace:` block.
- Added unit tests for single-error and chained-error formatting.

# Validation

- `cargo test -p agenthub cli_error -- --nocapture`
- `cargo test -p agenthub --test cli_error_output -- --nocapture`
- `git diff --check`
