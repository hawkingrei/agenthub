# Pyroscope Bootstrap Verification

## Summary

Closed the deployed Pyroscope bootstrap verification backlog item by locking the
role-scoped startup identity in a focused regression test and refreshing the
operator-facing documentation around main/node application names.

## Background

The original Pyroscope bootstrap rollout already established the process-wide
profiler crate, environment-variable gating, incomplete-configuration warning
path, non-fatal startup failure handling, and shutdown guard ownership. The
remaining P1 item was to verify the deployed contract stayed clear:

- full configuration starts one process-wide profiler agent;
- partial configuration warns and keeps the service running;
- shutdown drops the guard and stops the profiler cleanly.

## Scope

- `src/app.rs`
- `userdocs/docs/getting-started/configuration-basics.md`
- `docs/todo.md`

## Key Decisions

1. `agenthub::run()` still owns exactly one `_pyroscope` guard for the process
   lifetime, after tracing initialization and before `AppState::init()`.
2. Pyroscope startup options are now built by a small helper so main/node
   application names are testable without starting a real profiler.
3. Main-mode processes report as `agenthub.server`; node-mode processes report
   as `agenthub.node`.
4. The environment-gating contract remains in `agenthub-pyroscope`: all three
   required variables must be present and non-empty, partial configuration
   returns `None`, and the server continues.

## Validation

```bash
cargo test -p agenthub-pyroscope -- --nocapture
cargo fmt --check
git diff --check
npm --prefix userdocs run build
```

Attempted locally but blocked by host disk pressure during final link after
`target` had been rebuilt from a cold cache:

```bash
cargo test -p agenthub pyroscope_bootstrap_options_use_role_scoped_application_names -- --nocapture
```

The failure was `ld: write() failed, errno=28 (No space left on device)`, before
the focused test binary could run. PR CI should cover the main-crate regression
test on a clean runner.

## Follow-Ups

- None for the Pyroscope bootstrap contract. Future work for richer
  topology-specific tags should start from `docs/features/pyroscope-profiling.md`
  rather than reopening the bootstrap rollout.
