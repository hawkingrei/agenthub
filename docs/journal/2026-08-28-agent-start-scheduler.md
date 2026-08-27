# Agent Start Scheduler

Date: 2026-08-28

## Context

The daemon process supervisor establishes process ownership and ordered termination, but start
requests still needed resource admission, bounded waiting, an end-to-end deadline, and protection
against repeated executable spawn failures.

## Implementation

- Added one shared local-start semaphore to `AgentManager`.
- Kept queue waiting outside the supervisor lifecycle permit so shutdown is not coupled to queued
  work.
- Added separate queue and admitted-start deadlines.
- Added timeout cleanup that stops the current supervised session before persisting terminal agent
  and session failure state and releasing the per-agent reservation.
- Added per-agent exponential spawn-failure backoff that clears only after a successful process spawn.
- Made worktree Git preparation subprocesses kill-on-drop for cancellation safety.
- Added bounded `[agent_runtime]` configuration and startup logging of effective values.

## Validation Checklist

```bash
cargo test -p agenthub-config --lib agent_runtime_start_settings
cargo test -p agenthub --lib agent::manager::start_scheduler::tests -- --nocapture
cargo test -p agenthub --lib agent::manager::session::tests::start_timeout_persists_failure_before_releasing_reservation -- --exact --nocapture
cargo test -p agenthub --lib agent::manager::session::tests::spawn_failure_backoff_rejects_immediate_retry_without_respawn -- --exact --nocapture
cargo check -p agenthub --lib
cargo clippy -p agenthub --lib -- -D warnings
```

The targeted cases cover global admission timeout, permit reuse, growing and capped spawn backoff,
successful-spawn reset, durable timeout failure state, per-agent reservation release, and immediate
retry rejection without a second spawn.

## Follow-up

- Add durable delivery receipts for Team mailbox messages that cross the runtime boundary.
- Move remaining background workers and runtime watchers into one cancellation-aware task group.

## Canonical Contract

- [Agent Start Scheduling](../features/agent-start-scheduling.md)
