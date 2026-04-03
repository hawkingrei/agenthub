## Summary

- stabilize `idle_gc_checks_only_once_per_idle_window` in `agenthub-db`
- replace fixed cleanup sleeps with bounded condition waits for the positive cleanup assertions
- document the flaky timing root cause and validation

## Why

The test relied on a fixed `sleep(180ms)` after `record_activity()`, but idle GC runs in a spawned background task. Under slower CI environments, especially Bazel coverage, the cleanup can complete after that fixed sleep and cause a false failure.

## Testing

- `cargo test -p agenthub-db idle_gc_checks_only_once_per_idle_window -- --nocapture`
- `git diff --check`
