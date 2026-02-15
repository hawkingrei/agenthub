# A2A Team Review Follow-up: Orchestrator and Relay Hardening

## Summary

Address additional PR review findings for orchestrator scheduling robustness and
remote mailbox relay safety.

## Background

After the first review hardening pass, remaining comments focused on:

- orchestrator DAG/spec validation and dispatch robustness,
- relay request safety (header validation / timeout defaults / client limits),
- stronger relay signing metadata for receiver-side dedupe.

## Scope

- `src/team/orchestrator.rs`
- `src/team/manager/mailbox.rs`
- `src/team/manager/tests.rs`
- `docs/todo.md`

## Key Decisions

1. `parse_step_specs(...)` now validates:
   - unique `step_key`,
   - `entrypoint` exists in `steps` when `steps` is provided,
   - every `depends_on` reference exists,
   - dependency graph is acyclic (DFS cycle detection).
2. `bootstrap_run_steps_if_needed(...)` now adds per-step/run context on submit
   failure so diagnostics identify the exact failing step.
3. `dispatch_once(...)` no longer always re-fetches steps after reconcile;
   it re-fetches only when reconciliation changed step states.
4. `dispatch_once(...)` now re-fetches steps after each successful dispatch pass
   to avoid acting on stale state inside the same tick.
5. `dispatch_step(...)` now performs best-effort member stop when
   `start_step(...)` returns database/runtime errors (in addition to non-working
   status handling from previous patch).
6. `dispatch_once(...)` now short-circuits per-run dispatch after the first
   failed dispatch attempt in a tick and refreshes step snapshots before exit,
   preventing additional stale-step scheduling attempts in that tick.
7. Relay HTTP hardening:
   - configured `reqwest::Client` builder (connect timeout, redirect limit,
     idle pool cap),
   - default timeout when route timeout is omitted (`30s`, clamped to
     `[100ms, 60s]`),
   - strict header name/value validation (reject invalid names and CR/LF in
     values).
8. Relay HMAC signing now binds `message_id` explicitly and emits
   `X-AgentHub-Message-Id` header to help receiver-side anti-replay/idempotency
   strategies.
9. Orchestrator worker `spawn(...)` now returns `tokio::task::JoinHandle<()>`
   so callers can manage lifecycle explicitly (panic observation / coordinated
   shutdown wiring in future).
10. Remote relay adapter is now shared via process-level singleton (`OnceLock`)
   instead of per-tick reallocation, preserving `reqwest::Client` connection
   pool reuse.

## Validation

```bash
cargo fmt
cargo test parse_step_specs
cargo test team::orchestrator::tests::dispatch_step
cargo test team::orchestrator::tests::dispatch_once
cargo test team::manager::mailbox::tests::
cargo test remote_actor_messages_relay
cargo test dispatch_once_stops_run_dispatch_after_first_failure_in_tick
```

## Follow-ups

- Add receiver-side replay protection guidance (timestamp skew window +
  dedupe key policy) to remote relay integration docs.
- Evaluate whether relay worker should expose metrics for timeout and dead-letter
  reasons per route endpoint.
