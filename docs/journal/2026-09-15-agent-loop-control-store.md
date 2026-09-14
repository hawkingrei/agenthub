# Durable Loop Control Store

## Summary

Add domain records and an additive SQLite store for loop policies, pending activations, trigger
sources, execution reservations, and redacted lifecycle events. Trigger intake does not start a
process; the scheduler and recovery service are later slices.

## Background

Accepted work must survive provider and daemon exit. Existing watchdog and reminder fields cannot
authorize automatic startup, and message/task writes must have a transaction-compatible intake
boundary to avoid losing wakeups.

## Scope

- Explicit policy configuration with membership checks, revision comparison, and finite limits.
- Stable trigger receipts, conflicting-replay rejection, and bounded coalescing of source records.
- Scope-filtered activation/event reads and an intake entrypoint for existing store transactions.
- Shared domain dependency added to both Cargo and Bazel build targets.

## Key Decisions

- Migration creates no enabled policies from old process or watchdog state.
- Suspended actors retain and accept pending work; disabled actors reject new work while retaining
  existing receipts. Neither state change deletes outcomes or resets execution generations.
- Immediate work may coalesce; future deadlines remain separate so they cannot fire early.
- The tightest non-disabled Team pending limit applies to every producer, including more permissive
  members. Capacity errors occur before acceptance and preserve earlier work.
- Source references are typed identifiers; arbitrary prompt/tool payload fields are rejected.

## Validation

Focused commands:

- `cargo test -p agenthub-db --locked loop_ -- --nocapture`
- `cargo test -p agenthub-agent-domain --locked loop_`
- `cargo clippy -p agenthub-db -p agenthub-agent-domain --all-targets --locked -- -D warnings`
- `cargo fmt --all --check`

Coverage includes legacy migration/repetition, DB reopen, concurrent duplicate receipts, conflicting
replay, future-time isolation, scope/revision rejection, suspension, source/capacity bounds, atomic
rollback with a canonical write, and payload exclusion. These checks establish storage behavior;
they do not establish provider execution or crash recovery of a running process.

## Follow-Ups

Add durable admission and generation-fenced ownership, then structured outcome/cleanup recovery.
Remaining slices are tracked in [TODO](../todo.md#agent-loop-product-transition).
