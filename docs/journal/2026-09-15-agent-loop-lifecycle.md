# Loop Outcomes and Execution Cleanup

## Summary

Persist structured outcomes and self-continuations atomically, retain inspectable interruption on
uncertain execution, and connect configured actors' manual starts to durable writer reservations.
The finish RPC requires signed activation credentials. Provider launch, credential provisioning,
and actor CLI integration remain separate rollout gates.

## Background

A provider exit does not establish task completion. Likewise, an expired lease or a cached parent
exit status cannot establish that a process group has stopped writing. The lifecycle boundary must
preserve work arriving during finalization without allowing a replacement executor to overlap.

## Scope

- Structured outcomes, explicit wait reasons, idempotent finish receipts, and atomic continuations.
- Canonical task-note evidence for no-progress accounting, without changing task acceptance.
- Verified cleanup, bounded startup retries, interruption, source revocation, and canceled-task checks.
- Manual start/stop, caller disconnect, lease renewal, daemon shutdown, and startup compatibility.
- A bounded internal finish RPC that derives every identity field from authenticated claims.

## Key Decisions

- Finish records a `finalizing` outcome and continuation in one SQLite transaction. Replaying the
  same authenticated fence returns its original receipt even after cleanup. Conflicts fail.
- Revoke continuation sources without deleting their history or independent coalesced work.
  Recheck terminal task state at admission as well as finish.
- Credit a canonical task note only once and require the executing actor, Team, and execution time.
  Reporting `progress` alone never resets a budget; proposing completion never accepts the task.
- Cleanup keeps owner/generation checks after lease expiry. Startup failure retries reuse accepted
  work with capped exponential backoff; an executor exit without an outcome becomes interrupted.
- Startup marks expired executions interrupted but retains reservations with unknown process
  authority. Legacy bulk status cleanup excludes reserved actors/sessions. Explicit loop mailbox
  partition markers exclude those runs from legacy cancellation and linked-task reopening.
- Supervision registers a process before yielding after spawn. Daemon-owned start tasks survive
  caller disconnects, and shutdown waits for the complete startup/cleanup critical section.
- Lease-failure cleanup carries its original generation so delayed callbacks cannot stop a newer
  session. Linux cleanup kills and checks remaining supervised process-group members after a parent
  exit; zombie processes have no writer authority. This is process-group supervision, not containment
  of deliberately detached processes. Other platforms retain legacy behavior and fail loop preflight
  until equivalent cleanup evidence is available.

## Validation

Focused commands:

```bash
cargo build -p agenthub --bin agenthub --locked
cargo test -p agenthub-db -p agenthub-agent-domain --locked loop_
cargo test -p agenthub --lib --locked loop_
cargo test -p agenthub --lib --locked cancel_active_runs_on_startup
cargo test -p agenthub --lib --locked agent::manager::
cargo test -p agenthub --lib --locked internal::auth::tests
cargo clippy -p agenthub-db -p agenthub-agent-domain --all-targets --locked -- -D warnings
cargo clippy -p agenthub --all-targets --locked -- -D warnings
cargo fmt --all --check
```

Cases cover finish replay after reopen, conflicting and wrong-owner finish, capacity rollback,
canonical evidence reuse, canceled tasks, continuation revocation, work arriving around finalization,
expired reservations, bounded retries, manual reservation contention, stale cleanup, caller
disconnect, surviving process-group children, and explicit loop/legacy mailbox separation.
Results: 33 control-store and three domain loop tests; 96 manager tests; 17 root loop tests;
two legacy run-cancellation tests; and six authentication tests passed. The root loop selection
overlaps the manager selection. Clippy, formatting, relative documentation links, and exact
comparison with the existing `build.rs` protocol output also passed.

## Follow-Ups

Complete provider launch and actor CLI integration, including immutable configuration references,
stable mailbox binding, production generation-scoped credentials, and prompt delivery.
Due-time self-continuations are stored here; dependency/standing wait registration and event wakeup
remain their own slice. See [TODO](../todo.md#agent-loop-product-transition).
