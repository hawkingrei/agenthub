# Agent Start Scheduling

## Problem

Local agent starts can perform filesystem preparation, process spawning, ACP initialization, and
session recovery. Without global admission control, a burst of starts can exhaust daemon resources.
Without deadlines and failure backoff, blocked preparation or a repeatedly broken executable can
hold callers indefinitely or create a tight retry loop.

## Scope

- Globally bound concurrent local starts within one daemon process.
- Bound queue waiting separately from admitted start execution.
- Bound the complete admitted local-start attempt, including workspace preparation and ACP startup.
- Apply per-agent exponential backoff after process spawn failures.
- Persist terminal failure state before releasing the per-agent start reservation after a timeout.
- Cancel Git subprocesses when timed-out workspace preparation futures are dropped.

## Non-Goals

- Distributed admission control across multiple daemon instances.
- Limiting remote forwarding performed by a main node. The destination node schedules its own local
  process start.
- Retrying a failed start automatically without a new caller request.
- Defining a new public HTTP status-code contract for admission failures.
- Applying spawn backoff to workspace, database, ACP protocol, or configuration failures.

## Architecture

`AgentManager` owns one clone-safe `AgentStartScheduler`. The scheduler contains a global semaphore
and per-agent spawn-failure state. All `AgentManager` clones share both through `Arc` ownership.

The local start sequence is:

1. Reserve the agent ID with the existing duplicate-start guard.
2. Reject an active per-agent spawn backoff.
3. Wait for the global semaphore with the configured queue deadline.
4. Recheck spawn backoff after admission.
5. Acquire the process supervisor lifecycle permit.
6. Run the complete local start under the configured start deadline.
7. On timeout, stop the tracked supervised session, remove matching in-memory handles, persist the
   failed session and agent state, and only then release the per-agent reservation.

Queue waiting intentionally occurs before acquiring the supervisor lifecycle permit. Daemon
shutdown can therefore close the lifecycle gate without waiting for queued starts. A waiter admitted
after shutdown begins fails when it attempts to acquire that gate.

Each start attempt publishes its generated session ID to a timeout cleanup tracker before workspace
preparation begins. Resume fallback replaces the tracked ID before starting its next attempt, so
cleanup always targets the current launch.

## Contracts

### Admission

- The default concurrent local-start limit is 4.
- The default queue timeout is 30 seconds.
- The default admitted-start timeout is 120 seconds.
- A queued request retains the existing per-agent start reservation, so another request for the same
  agent fails instead of entering the queue.
- Remote forwarding does not consume a main node's local-start permit.

### Timeout Cleanup

- A timeout response is not returned until process cleanup and durable failure-state persistence
  complete.
- Cleanup stops the current session through `AgentProcessSupervisor`, preserving process-tree
  termination semantics.
- A session row is inserted or updated to `failed` with `ended_at` populated.
- The agent row is updated to `failed` after the supervised process has exited.
- If process termination or failure-state persistence fails, the caller receives a cleanup error and
  AgentHub does not falsely claim that an unconfirmed process is terminal.
- A late timeout cannot remove an in-memory handle for a newer session because removal compares the
  tracked session ID.

### Spawn Failure Backoff

- Only `AgentExecutor::spawn_process` failures increment backoff.
- The default initial delay is 250 milliseconds and the default maximum is 30 seconds.
- Delay doubles after each failed spawn and saturates at the configured maximum.
- Failure count remains across elapsed retry windows and clears immediately after a successful spawn.
- Admission checks backoff both before queueing and after acquiring a permit.

### Configuration

The optional `[agent_runtime]` section accepts:

| Setting | Default | Accepted effective range |
| --- | ---: | ---: |
| `start_max_concurrent` | `4` | `1..=64` |
| `start_queue_timeout_seconds` | `30` | `1..=300` |
| `start_timeout_seconds` | `120` | `1..=900` |
| `spawn_backoff_initial_millis` | `250` | `10..=60000` |
| `spawn_backoff_max_millis` | `30000` | `initial..=300000` |

Values outside these ranges are clamped. Effective values are logged during daemon startup.

## Validation Matrix

| Scenario | Expected result |
| --- | --- |
| One permit is held and another agent queues | The second request fails after the queue deadline |
| A permit is released before the queue deadline | The next request is admitted |
| A local executor never completes spawn | The start times out, reservation is released, and durable agent/session state is `failed` |
| Spawn fails and the caller retries immediately | The retry is rejected without another spawn attempt |
| Spawn fails again after the first delay | The next delay doubles up to the configured maximum |
| Spawn succeeds after earlier failures | Backoff state is cleared |
| Workspace Git preparation is canceled | Its Git subprocess is killed on drop |
| Daemon shutdown begins while starts are queued | Queued starts do not hold the supervisor lifecycle gate |

## Operational Notes

- Repeated `agent spawn backoff active` errors usually indicate a missing or broken agent executable.
- `agent start queue timed out` indicates local admission saturation; increase concurrency only after
  checking CPU, memory, filesystem, and provider startup pressure.
- `agent start timed out` covers the complete admitted local start, not only OS process creation.
- Configuration changes take effect after daemon restart.

## Open Risks

- Backoff state is process-local and resets on daemon restart.
- Admission errors currently use existing API error mapping rather than a dedicated overload status.
- Non-Git workspace preparation code must independently preserve cancellation safety.

## Source Journals

- [2026-08-28 Agent Start Scheduler](../journal/2026-08-28-agent-start-scheduler.md)
