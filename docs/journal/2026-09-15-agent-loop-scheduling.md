# Bounded Loop Follow-Ups And Standing Triggers

## Summary

Add durable due times, recurring timers, task conditions, and thread watches above ordinary loop
intake. Actor and owner interfaces register, inspect, and revoke offline future work.

## Background

The work-event slice recovers immediate Team work after process exit. Future work additionally needs
an atomic dependency observation and bounded catch-up, without a provider polling loop.

## Scope

- Registration and firing storage, canonical transaction hooks, and bounded daemon reconciliation.
- Task/parent cancellation, source-aware revocation, and scope/history guards.
- Signed actor RPC/CLI and owner HTTP registration, pagination, firing history, and revocation.
- Canonical contract: [Agent Loop Scheduling](../features/agent-loop-scheduling.md).

## Key Decisions

- Record dependency edges and replies in canonical write transactions, including legacy run-status
  synchronization. Register and read current dependency state under the same SQLite write lock.
- Keep one pending firing per registration. Preserve cursor ranges and use ordinary intake budgets;
  capacity or disablement retains the pending observation for a five-second retry.
- Process at most 32 registrations per tick. Collapse missed timer intervals arithmetically into one
  firing, and retain standing limits and durable no-progress counters across restarts.
- Add one future-work CLI-help pointer to the activation entry prompt (`loop-entry-v3`), within the
  existing 1500-byte bound. This is a runtime recovery pointer; role prompts and skills stay gated on
  later tool integration.
- Derive provenance from authenticated entrypoints. Business retries preserve original attribution;
  scheduling does not grant task ownership or roster authority.
- Revoke sources, not independently coalesced work. Cancellation fences tools while retaining the
  process reservation until cleanup; normal finish preserves future registrations.

## Validation

- `cargo test -p agenthub-db loop_runtime --locked --offline`: all 54 loop tests passed, including
  12 new scheduling cases with file reopen, dependency races/reversion, thread cursors, disabled and
  capacity retention, suspension, fan-out, revocation races, descendants, and scope history.
- `cargo test -p agenthub --lib --locked --offline`: all 870 root library tests passed, including
  six new scheduling tests and the existing offline work/recovery regressions. Local IPC fixtures
  used normal socket access; loopback HTTP tests bypassed the inherited proxy through command-local
  `NO_PROXY` and `no_proxy` settings.
- The real binary/fake ACP cycle used signed CLI calls over local gRPC for register/list/show/revoke
  and structured finish. Four separate worker/coordinator sessions completed; both actors retained
  two no-progress outcomes, and admission rejected the next worker wake. No provider stayed alive
  to wait, and temporary revoked registrations did not fire.
- `cargo clippy -p agenthub -p agenthub-db --all-targets --locked --offline -- -D warnings`,
  `cargo fmt --all --check`, explicit formatting of the included HTTP test, `git diff --check`,
  generated protobuf equality, and relative documentation links passed.
- The activation entry pointer is versioned as `loop-entry-v3`, 1037 bytes under the existing
  1500-byte guard. Command help owns the procedure and request examples.
- Local Bazel and paid-provider smoke were not run. CI validates Bazel and coverage.

## Follow-Ups

Slices 9–18 remain: shared MCP operation journal, scoped knowledge bootstrap, role prompts, history
and UI, app integration, and the Rara protocol/lifecycle track. No merge or deployment is performed
by this checkpoint. Live-provider smoke and CI remain separate from local deterministic fixtures.
