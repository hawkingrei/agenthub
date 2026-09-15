# Fenced Loop Admission

## Summary

Add atomic admission and renewable actor reservations to the loop control store. Pending work
is admitted only under current membership, execution policy, ownership, due time, and finite budgets.
The following lifecycle slice connects these methods to process supervision and daemon recovery.

## Background

In-memory start guards cannot preserve ownership across process exit. A durable lease also cannot
stop a filesystem writer, so an expired reservation must remain held pending verified cleanup.

## Scope

- Atomic activation claim, per-actor generations, renewable leases, and explicit manual reservations.
- Actor/Team rolling limits, startup/no-progress ceilings, and foreign task-claim deferral.
- Bounded fair candidate scans, deduplicated deferral events, and additive migration from the store slice.

## Key Decisions

- Automatic and explicit manual execution use one reservation API. Wiring existing manual startup
  into this boundary belongs with lifecycle cleanup so failed starts cannot leak reservations.
- Expiry never releases execution. Store `lease_expired_unfenced` until a supervisor supplies cleanup
  evidence; reject stale generation/owner renewal.
- Lease duration is captured by the reservation. Later policy edits do not mutate an active lease's
  renewal duration, while suspension still blocks new automatic admission.
- Use persisted admission events for rolling budgets and retain no-progress/startup counters across
  restart. Outcome and failed-start paths will update those counters in the lifecycle slice.
- Advance deferred admission checks by five seconds, independently of source due time, with at most
  32 candidates per scan. Repeated unchanged deferrals share one trace finding.

## Validation

- `cargo test -p agenthub-db -p agenthub-agent-domain --locked loop_ -- --nocapture`
- `cargo clippy -p agenthub-db -p agenthub-agent-domain --all-targets --locked -- -D warnings`
- `cargo fmt --all --check`

Focused cases cover concurrent claim, manual/automatic reservation races, expired/stale renewal,
future deadlines, suspension, configuration snapshots, durable limits, Team budget bypass attempts,
fairness behind 64 deferred items, task claims, membership changes, and upgrade from the preceding
storage schema. The upgrade fixture reopens the database at the daemon restart boundary before
applying migrations; it does not reuse statements prepared before its schema downgrade.

These checks establish control-store admission. They do not prove process-tree cleanup or a complete
provider loop; those remain explicit requirements of the next slices.

## Follow-Ups

Implement structured finish and verified cleanup/restart, including manual start/stop reservations,
failed-start backoff, and outcome-driven no-progress accounting. Then connect provider launch and
task/IM recovery. See [TODO](../todo.md#agent-loop-product-transition).
