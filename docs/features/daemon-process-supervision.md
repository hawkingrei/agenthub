# Daemon Process Supervision

## Problem

The daemon owns long-lived local agent subprocesses, but runtime state previously lived only in
per-agent handles. Shutdown marked SQLite sessions as exited before child termination, explicit stop
killed only the direct process, and dropping a Tokio child did not prove that descendants had exited.
That ordering could leave live provider or tool processes behind while the control plane reported a
terminal state.

## Scope

- Define daemon ownership for every local agent process from spawn through final reap.
- Stop the entire process tree with a bounded graceful deadline and force-kill fallback.
- Cover processes that are still starting and have not yet entered the active handle map.
- Serialize the final shutdown snapshot against concurrent local starts.
- Persist shutdown or cancellation terminal state only after process exit is proven.

## Non-Goals

- Replacing ACP as the provider integration boundary.
- Moving provider workers into the daemon process.
- Changing remote-node process ownership; a remote daemon supervises its own local children.
- Defining global start admission, retry backoff, durable mailbox delivery receipts, or unified task
  cancellation. Those are separate runtime-reliability phases.

## Architecture

`AgentProcessSupervisor` is daemon-local and shared by `AgentManager` and `LocalExecutor`.

1. `LocalExecutor` wraps every command with kill-on-drop plus a Unix process group or Windows Job
   Object before spawn.
2. The supervisor immediately registers the child by AgentHub session ID, before session setup can
   fail or be cancelled.
3. A pending registration is committed only after the active `AgentHandle` is installed. Dropping an
   uncommitted registration schedules bounded stop and reap.
4. Natural exit, explicit stop, and daemon shutdown all converge on the same supervised child.
5. Shutdown takes the exclusive lifecycle gate, preventing new local starts and waiting for in-flight
   starts to publish their registrations before `stop_all()` snapshots the registry.
6. SQLite terminal state is written after every targeted process tree has exited and been reaped.

The supervisor uses a shared child mutex to make natural-exit polling, explicit stop, pending-start
cleanup, and daemon shutdown mutually exclusive for one process.

## Contracts

### Ownership Contract

- Every successfully spawned local child is registered under its runtime session ID immediately.
- A start future cannot abandon an uncommitted process; registration drop triggers cleanup.
- The registry entry remains until natural exit finalization or a successful supervised stop.
- A daemon shutdown failure must retain non-terminal database state rather than claim an unproven
  exit.

### Stop Contract

- Unix children run as leaders of dedicated process groups; Windows children run in dedicated Job
  Objects.
- Graceful stop sends `SIGTERM` to the Unix process group and waits up to five seconds.
- If the graceful deadline expires, the supervisor force-kills the process group or Job Object and
  waits up to five seconds.
- A final kill-and-wait pass acts as the orphan reaper. A stop succeeds only when the wrapped process
  tree wait completes.
- All registered processes are stopped concurrently during daemon shutdown.

### State Ordering Contract

- Explicit `stop_agent` writes `cancelled`/`stopped` only after supervised exit.
- Daemon shutdown bulk-marks remaining running sessions and agents as `exited` only after
  `stop_all()` succeeds.
- Natural exits keep their existing `completed`/`failed` finalization path.
- If process exit cannot be proven, the stop call returns an error and does not write a terminal
  state for that stop path.

### Lifecycle Gate Contract

- Local starts hold a shared lifecycle permit until their active handle and committed registration
  exist or startup fails.
- Shutdown holds the exclusive lifecycle permit for the rest of daemon teardown.
- Once shutdown begins, new local start permits are rejected.

## Validation Matrix

| Boundary | Required evidence |
| --- | --- |
| Process-tree escalation | A child tree that ignores `SIGTERM` crosses the graceful deadline, is force-killed, and its descendant PID no longer exists. |
| State ordering | A process that delays its `SIGTERM` exit leaves its session `running` during the delay and becomes `exited` only after supervised shutdown completes. |
| Start/shutdown race | Shutdown waits for an active start permit and every later start-permit request fails. |
| Existing explicit stop | Existing stop regression coverage still proves idle-GC cleanup after the child exits. |
| Build boundaries | `cargo fmt`, focused Cargo tests, Cargo check, and the relevant Bazel library/test targets pass on the exact change head. |

## Operational Notes

- The graceful and force deadlines are per process tree; `stop_all()` waits for trees concurrently.
- A process that deliberately creates a new OS session escapes ordinary process-group membership.
  Provider and tool subprocesses must not detach from the daemon-owned group/job boundary.
- Shutdown errors are intentionally loud and leave recoverable `running` rows for startup
  reconciliation instead of publishing a false terminal state.
- Built-in ACP workers remain child processes of `agenthubd`; this contract does not add release
  binaries.

## Open Risks

- Daemon instance locking and generation fencing are not yet implemented, so two daemon processes
  can still contend for one database/node identity.
- Start admission is still per-agent rather than globally bounded, and spawn failures have no shared
  backoff policy.
- Team mailbox runtime delivery still lacks durable delivery receipts.
- Background tasks still use independent Tokio tasks rather than one cancellation-aware daemon task
  group.
- Cross-platform CI must exercise the Windows Job Object path; local Unix tests cover process groups.

## Source Journals

- [Daemon process supervision](../journal/2026-08-28-daemon-process-supervision.md)
- [Two-binary runtime consolidation](../journal/2026-08-27-two-binary-runtime-consolidation.md)
