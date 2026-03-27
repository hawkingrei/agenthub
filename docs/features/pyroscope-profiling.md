# Pyroscope Profiling

## Problem

AgentHub does not currently expose a built-in continuous profiling path for backend runtime debugging.
Operators need an opt-in integration that can be enabled at process start without changing normal
application behavior when profiling is not configured.

## Scope

- Process-wide Pyroscope bootstrap for the main AgentHub server binary.
- Environment-variable driven enablement using the existing Grafana Pyroscope Rust client.
- Startup and shutdown lifecycle contract for the profiler worker threads.
- User-facing and operator-facing documentation for the enablement contract.

## Non-Goals

- Per-agent or per-Team profiling controls.
- Runtime reconfiguration without restarting the process.
- A full configuration-file surface for profiling secrets.
- Profiling UI changes inside AgentHub.

## Architecture

### 1) Dedicated Bootstrap Crate

`crates/agenthub-pyroscope` owns:

- required environment-variable detection;
- startup gating and incomplete-config warnings;
- `pyroscope-rs` + `pprof-rs` backend wiring;
- shutdown guard ownership for the running profiler agent.

The main `agenthub::run()` path only asks the crate to maybe start profiling and keeps the returned
guard alive for the process lifetime.

### 2) Enablement Contract

Profiling starts only when all three environment variables are present with non-empty values:

- `PYROSCOPE_SERVER_ADDRESS`
- `PYROSCOPE_BASIC_AUTH_USER`
- `PYROSCOPE_BASIC_AUTH_PASSWORD`

If none are present, profiling stays disabled.
If only a subset is present, AgentHub logs a warning and continues without profiling.
Non-UTF8 values are treated as missing so bootstrap never panics while inspecting the process
environment.

### 3) Runtime Lifecycle

- Bootstrap happens after tracing initialization so startup decisions are logged.
- Profiling startup failure is non-fatal; AgentHub logs the error and continues serving traffic.
- On process shutdown, the guard stops the running agent, flushes the final profile snapshot, and
  shuts down the profiler threads.

### 4) Profiling Identity

- Application name: `agenthub.server`
- Backend: `pyroscope-rs` `backend-pprof-rs`
- Sample rate: `100`
- Static tags:
  - `service=agenthub`
  - `version=<agenthub crate version>`

## Contracts

### 1) Startup Contract

- AgentHub must not attempt to profile actor/doctor CLI early-exit flows.
- The server runtime should only start one process-wide profiler agent.

### 2) Failure Contract

- Missing or partial environment configuration must not abort the process.
- Non-UTF8 environment values must not abort the process and should be treated the same as missing
  configuration.
- Startup failures from the upstream profiler library must be observable via structured logs.

### 3) Secret Handling Contract

- Password values must never be written to logs.
- Visible logs may include the server address and basic-auth username for debugging.

## Validation Matrix

- `cargo test -p agenthub-pyroscope`
- `cargo test -p agenthub log_config_details_handles_all_branches -- --nocapture`
- `cargo check -p agenthub`

## Operational Notes

- This surface is intentionally environment-driven because it carries operational secrets and should
  remain easy to enable in deployment manifests.
- Profiling requires a process restart because bootstrap happens once during server startup.

## Open Risks

- The initial rollout uses a fixed application name and tag set; future multi-node deployments may
  want additional topology tags.
- The upstream Pyroscope client uses its own internal `log` instrumentation, which AgentHub does
  not currently bridge into `tracing`.

## Source Journals

- `docs/journal/2026-03-26-agenthub-pyroscope-bootstrap.md`
