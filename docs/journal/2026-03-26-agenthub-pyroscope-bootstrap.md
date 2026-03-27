# 2026-03-26 AgentHub Pyroscope Bootstrap

## Summary

- added a dedicated `agenthub-pyroscope` crate for process-wide profiler bootstrap;
- integrated `pyroscope-rs` with the `backend-pprof-rs` backend into the main server startup path;
- enabled profiling only when all required `PYROSCOPE_*` variables are present;
- documented the runtime contract and rollout verification backlog.

## Implementation Notes

- bootstrap happens in `agenthub::run()` after tracing initialization and before `AppState::init()`;
- incomplete environment configuration logs a warning and leaves profiling disabled;
- environment inspection reads only the required keys and treats non-UTF8 values as missing so
  profiling bootstrap remains non-fatal;
- profiler startup failure is non-fatal so deployment issues do not block the main service;
- process shutdown drops a guard that stops the running profiler agent and shuts down its worker threads.

## Required Environment Variables

- `PYROSCOPE_SERVER_ADDRESS`
- `PYROSCOPE_BASIC_AUTH_USER`
- `PYROSCOPE_BASIC_AUTH_PASSWORD`

## Validation

- local:
  - `cargo test -p agenthub-pyroscope`
  - `cargo test -p agenthub log_config_details_handles_all_branches -- --nocapture`
  - `cargo check -p agenthub`
- follow-up:
  - verify one real deployment with the required variables set and confirm profile ingestion reaches the configured Pyroscope server
  - verify partial environment configuration only emits a warning and does not stop HTTP startup
