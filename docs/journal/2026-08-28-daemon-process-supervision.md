# Daemon Process Supervision

## Summary

Added daemon-owned local agent process supervision with process-tree shutdown, bounded graceful and
force deadlines, pending-start cleanup, and database terminal-state ordering after verified exit.

## Background

Local agent children were stored only in `AgentHandle`. Daemon shutdown updated all running rows to
`exited` without first stopping children, while explicit stop updated the session and agent before
killing only the direct Tokio child. Since Tokio children continue running when dropped by default,
startup cancellation and daemon teardown could leave provider descendants alive beyond the state
that claimed to own them.

## Scope

- Added `AgentProcessSupervisor` as the owner of spawned local process registrations.
- Wrapped Unix children in dedicated process groups and Windows children in Job Objects.
- Added `SIGTERM`, deadline, force-kill, and final reaper behavior.
- Added an exclusive shutdown gate against concurrent local starts.
- Reordered explicit stop and daemon shutdown persistence after verified child exit.
- Preserved ACP provider/session behavior and the two-native-binary artifact contract.

## Key Decisions

- Kept ACP as the stable provider boundary instead of introducing provider-specific runtime drivers.
- Registered processes immediately at spawn, before ACP/session initialization, so cancellation of a
  pending start remains recoverable by the supervisor.
- Used a pending-registration RAII guard. A registration becomes durable only after the active handle
  is installed; otherwise guard drop schedules stop and reap.
- Kept database rows non-terminal when shutdown cannot prove that all targeted process trees exited.
- Used the existing transitive `process-wrap` version as a direct dependency to share its Unix process
  group and Windows Job Object behavior.

## Validation

Executed on the implementation worktree:

```bash
cargo fmt --all
cargo check -p agenthub --lib
cargo clippy -p agenthub --lib -- -D warnings
cargo test -p agenthub agent::manager::supervisor::tests::shutdown_waits_for_active_start_and_rejects_new_starts -- --exact
cargo test -p agenthub agent::manager::supervisor::tests::stop_all_force_kills_and_reaps_process_groups -- --exact
cargo test -p agenthub agent::manager::runtime::tests::shutdown_marks_terminal_state_only_after_process_tree_exits -- --exact
cargo test -p agenthub agent::manager::runtime::tests::stop_agent_removes_idle_gc_state_even_when_exit_watcher_exits_early -- --exact
cargo test -p agenthub --lib
bazel build //:agenthub_lib
```

The focused process-tree test proved force escalation and descendant reaping. The shutdown ordering
test observed `running` while the child delayed exit, then `exited` after `stop_all_on_shutdown()`.
The existing explicit-stop regression remained green. The full library suite passed 779 tests and
failed three pre-existing `state::tests::initialize_services_*` cases before reaching daemon behavior;
all three panic inside `lance-namespace-impls 8.0.0` because the directory-manifest dataset enables
old-version cleanup. Re-running those three serially produced the same dependency assertion.

The Bazel command reached the unchanged `lance-file 8.0.0` build script and then failed before
compiling the root target because the sandbox could not find `protoc`, even though the host has
`/opt/homebrew/bin/protoc`. No Bazel configuration was changed for this checkpoint. Exact-head Bazel
success remains a follow-up after the repository supplies the build script with a hermetic `protoc`
tool, as it already does for the patched Codex code-mode protocol crate.

## Follow-Ups

- Add database-path plus node-ID instance locking and daemon generation fencing.
- Add globally bounded start admission, timeout, and spawn-failure backoff.
- Add durable Team mailbox runtime delivery receipts.
- Keep the unified cancellation-aware task-group contract documented in
  [Daemon Task Lifecycle](../features/daemon-task-lifecycle.md).
- Run exact-head Bazel validation and cross-platform CI, including Windows Job Object coverage.
