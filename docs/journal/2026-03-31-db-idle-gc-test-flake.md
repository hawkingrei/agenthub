## Summary

Stabilized the `idle_gc_checks_only_once_per_idle_window` test in `agenthub-db`.

## Why

The test previously assumed that a fixed `sleep(180ms)` was always long enough for the background idle-GC task to wake up, acquire state, open the per-agent SQLite pool, and finish cleanup. That assumption was too brittle under slower CI environments such as Bazel coverage runs.

## Change

- Replaced the fixed post-activity sleep used for the cleanup assertions with a small polling helper that waits until the expected old-event count is observed or a bounded timeout expires.
- Kept the "should not re-run without new activity" assertion on the existing short idle window so the test still validates the generation-gating behavior rather than masking it behind long unconditional waits.

## Validation

- `cargo test -p agenthub-db idle_gc_checks_only_once_per_idle_window -- --nocapture`
