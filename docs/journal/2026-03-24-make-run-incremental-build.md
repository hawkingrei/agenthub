# Summary

Reduce `make run` local rebuild cost by avoiding workspace-wide Rust builds and
by making the web build target incremental.

# Why

The previous `Makefile` path always did two expensive things:

- `run-server` rebuilt the entire Rust workspace before `cargo run`
- `build-web` was marked phony, so `make run` always rebuilt the frontend even
  when `web/dist` was already up to date

For normal local development, that was much heavier than necessary.

# What Changed

- `build` now builds only:
  - `agenthub`
  - `agenthub-codex-acp`
- `run-server` now:
  - ensures `agenthub-codex-acp` is built
  - runs `agenthub` with `cargo run -p agenthub --`
- `build-web` now uses a stamp file at `web/dist/.build-stamp`
  and tracks the main frontend inputs, so repeated `make run` calls do not
  rebuild the frontend unless the web sources changed

# Validation

- `make build-web`
- `make -n build`
- `make -n run-server`

Observed result after the stamp was created:

- `make -n build` prints only `cargo build -p agenthub -p agenthub-codex-acp`
- `make -n run-server` no longer includes `npm run build` when the frontend is
  already up to date
