# Daemon Task-Group Shutdown

## Summary

Moved daemon-lifetime Tokio workers and agent-runtime watchers into one cancellation-aware task group
with ordered phases, bounded joins, registration fencing, and aggregated teardown failures.

## Background

The process supervisor established reliable child-process ownership, but daemon background workers
and runtime watchers were still detached independently. Shutdown had no complete inventory to cancel
or join, and a panic, unexpected worker exit, or hung task could be invisible to the final result.

## Scope

- Added background and runtime phases backed by `CancellationToken` and `TaskTracker`.
- Registered internal gRPC, mailbox, delivery, relay, trigger, storage-maintenance, backfill, and
  permission-fallback work in the background phase.
- Registered agent output readers, exit watchers, and loop controllers in the runtime phase.
- Quiesced public HTTP before background cancellation.
- Preserved process shutdown between the background and runtime joins.
- Aggregated worker, process, runtime, and public-server shutdown failures.

## Key Decisions

- Kept two ordered phases inside one task-group abstraction because exit watchers must remain active
  while supervised processes stop, whereas delivery and ingress workers must stop first.
- Classified request-owned WebSocket/SSE forwarding and bounded archive writes outside the daemon
  group; their abort/timeout boundaries are local to the request or side effect.
- Rejected task registration under the same lock that closes the tracker, eliminating a spawn versus
  shutdown race.
- Recorded normal completion as failure only for daemon workers. Finite jobs and runtime watchers may
  complete successfully before shutdown.
- Continued later cleanup phases after earlier failures so one bad worker cannot prevent process
  reaping.

## Validation Scope

```bash
cargo fmt --all
cargo check -p agenthub --lib
cargo clippy -p agenthub --lib -- -D warnings
cargo test -p agenthub --lib daemon_tasks::tests -- --nocapture
cargo test -p agenthub --lib agent::manager::process::tests -- --nocapture
cargo test -p agenthub --lib agent::manager::runtime::tests::shutdown_marks_terminal_state_only_after_process_tree_exits -- --exact
cargo test -p agenthub --lib
bazel build //:agenthub_lib
```

Focused task-group coverage checks ordered cancellation, task-count drain, panic reporting, and the
post-shutdown registration fence. Existing process and worker tests cover the integrated runtime
paths. Exact-head CI remains the authority for repository-wide and cross-platform validation.

## Follow-Ups

- Consider fail-fast daemon shutdown when a required background worker exits unexpectedly.
- Keep new detached tasks classified explicitly as request-scoped, bounded side effects, process
  supervisor cleanup, or daemon-task-group members.
- Retain cross-platform coverage for task shutdown plus process-tree teardown.

