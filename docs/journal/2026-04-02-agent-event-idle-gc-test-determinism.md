# Agent Event Idle GC Test Determinism

## Summary

`crates/agenthub-db/src/lib.rs::tests::idle_gc_checks_only_once_per_idle_window` could fail nondeterministically even when the idle-gc scheduler itself only fired once.

The flaky edge was in the test signal, not in the once-per-idle-window gate.

## Root Cause

The test used old-row count as a proxy for "the previous idle-gc run has fully finished":

1. insert one old row;
2. call `record_activity`;
3. wait until old-row count becomes `0`;
4. insert a second old row without new activity;
5. assert the second row stays present.

That proxy is racy because `cleanup_agent_event_history()` deletes in a loop. The first time the count reaches `0`, the spawned cleanup task may still be inside the same cleanup run and can legally observe and delete a newly inserted old row before it exits.

So the failure did not prove that idle-gc scheduled a second run. It only proved that the test inserted another old row before the first cleanup task had fully completed.

## Change

The idle-gc state now tracks `completed_generation` for the generation that actually finished its cleanup attempt.

The test waits for generation `1` to complete before inserting the second old row, so the assertion now measures the intended property directly:

- without new activity, no new idle-gc generation is scheduled;
- the second old row remains until another `record_activity()` starts the next idle window.

## Validation

- `cargo test -p agenthub-db idle_gc_checks_only_once_per_idle_window -- --nocapture`
- `cargo test -p agenthub-db cleanup_agent_event_history_deletes_in_multiple_batches -- --nocapture`
