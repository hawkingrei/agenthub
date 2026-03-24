# Summary

Reduce `make run` local rebuild cost by avoiding workspace-wide Rust builds
while keeping the frontend build path unchanged.

# Why

The previous `Makefile` path rebuilt the entire Rust workspace before
`cargo run`.

For normal local development, that was much heavier than necessary.

# What Changed

- `build` now builds only:
  - `agenthub`
  - `agenthub-codex-acp`
- `run-server` now:
  - ensures `agenthub-codex-acp` is built
  - runs `agenthub` with `cargo run -p agenthub --`
- `build-web` intentionally remains phony because the frontend build cost is
  acceptable and always rebuilding avoids stale asset edge cases from partial
  dependency tracking

# Validation

- `make -n build`
- `make -n run-server`

Observed result:

- `make -n build` prints only `cargo build -p agenthub -p agenthub-codex-acp`
- `make -n run-server` still builds the frontend and no longer performs a
  workspace-wide Rust build
