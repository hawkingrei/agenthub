# Daemon Task Lifecycle

## Problem

The daemon previously detached long-lived Tokio tasks independently. Shutdown stopped supervised
agent processes, but it did not have one cancellation boundary or join point for ingress workers,
delivery workers, agent output readers, exit watchers, and loop controllers. A task could therefore
continue using runtime state while shutdown was publishing terminal process state, and worker panics
or join timeouts were not reported as daemon teardown failures.

## Scope

- Own daemon-lifetime background workers and agent-runtime watchers in one task group.
- Separate background ingress from tasks that must remain alive while supervised processes stop.
- Reject new tracked tasks after the corresponding shutdown phase begins.
- Cancel and join each phase with a bounded deadline.
- Surface task failures, panics, unexpected worker exits, and join timeouts.
- Preserve the process-tree and database-state ordering contract.

## Non-Goals

- Treating request-scoped WebSocket or SSE forwarding as daemon-lifetime work.
- Making bounded message-archive dual writes part of daemon shutdown.
- Replacing process supervision with Tokio task cancellation.
- Adding provider-specific runtime ownership or release binaries.
- Automatically restarting a failed background worker.

## Architecture

`DaemonTaskGroup` is shared through `AgentManager` and owns two ordered `TaskTracker` phases with
independent `CancellationToken` values.

The background phase owns daemon ingress and control-plane work:

- internal gRPC serving;
- Team mailbox unread hints and durable runtime delivery;
- remote mailbox relay;
- time-trigger dispatch;
- message-body outbox draining and startup backfill;
- message-index read repair when enabled;
- delayed permission-review fallback jobs.

The runtime phase owns tasks coupled to a supervised local agent process:

- stdout and stderr readers;
- child-exit watchers;
- agent-loop controllers.

Public HTTP graceful shutdown remains the outer ingress gate. After it quiesces, cleanup cancels and
joins the background phase, stops and reaps every supervised agent process, then cancels and joins
the runtime phase. Node mode follows the same task/process ordering without public HTTP.

Request-scoped WebSocket/SSE tasks retain request-owned abort handles. Archive dual writes remain
bounded, best-effort side effects with their own timeouts. Pending process-registration cleanup
remains under `AgentProcessSupervisor`; the shutdown lifecycle gate and supervisor registry provide
its process-level join boundary.

## Contracts

### Registration Contract

- Daemon-lifetime work must enter through `spawn_background_worker`, `spawn_background_job`, or
  `spawn_runtime_task`.
- A worker is expected to run until cancellation; normal completion before cancellation is a
  teardown failure.
- A job or runtime watcher may complete normally before shutdown.
- A phase rejects registration atomically once shutdown begins.

### Shutdown Contract

The main daemon performs these steps in order:

1. stop accepting public HTTP and drain requests up to the HTTP deadline;
2. cancel and join background ingress/control tasks;
3. stop and reap supervised local process trees;
4. cancel and join remaining runtime readers, watchers, and loop controllers.

Each task phase has a five-second join deadline. A timeout is reported with the remaining task count;
the daemon does not claim clean shutdown.

### Failure Contract

- Task errors, panics, unexpected worker exits, and join timeouts are retained by the owning phase.
- Cleanup attempts every later phase even if an earlier phase fails.
- Public HTTP failure and cleanup failure are aggregated so cleanup evidence is not discarded.
- Cancellation-aware workers return success after observing their phase token.

### Process Boundary Contract

- Tokio cancellation does not establish process exit.
- `AgentProcessSupervisor` remains the authority for process-tree termination and reaping.
- Runtime watchers stay active until supervised process shutdown finishes, then receive runtime
  cancellation as the final drain boundary.

## Validation Matrix

| Boundary | Required evidence |
| --- | --- |
| Phase ordering | Background cancellation completes while runtime tasks remain tracked; runtime cancellation follows separately. |
| Registration fence | A phase rejects a new task after shutdown begins. |
| Failure reporting | A tracked panic includes the task name and panic payload in the shutdown error. |
| Runtime integration | Output readers, exit watchers, and loop controllers compile and retain focused behavior coverage through the task group. |
| Worker integration | Startup workers and internal gRPC register successfully and stop through cancellation. |
| Process ordering | Existing supervised shutdown coverage still proves process exit before terminal database state. |
| Build boundaries | Formatting, focused Cargo tests, Cargo check/clippy, and relevant Bazel targets are evaluated on the exact change head. |

## Operational Notes

- Task names include the worker kind and runtime identifiers where available so shutdown errors are
  actionable.
- The join deadline is a daemon-wide safety bound per phase, not a per-task delay.
- Cancellation remains cooperative. A task that ignores its token produces a timeout and the Tokio
  runtime tears it down only when the daemon exits.
- Normal completion is valid for finite background jobs and runtime watchers, but not for workers
  whose contract is to remain available until cancellation.

## Open Risks

- Failures are logged when observed and returned during shutdown; an unexpected worker exit does not
  yet trigger immediate daemon-wide fail-fast shutdown.
- Request-scoped and bounded side-effect tasks intentionally use separate ownership. New detached
  tasks require an explicit lifecycle classification during review.
- Cross-platform CI still needs to exercise unified task shutdown together with Windows process Job
  Object teardown.

## Source Journals

- [Daemon task-group shutdown](../journal/2026-08-28-daemon-task-group.md)
- [Daemon process supervision](../journal/2026-08-28-daemon-process-supervision.md)

