---
sidebar_position: 6
---

# Session Lifecycle

An AgentHub agent is a durable configuration plus a backend-managed runtime.
Browser tabs observe and control that runtime; they do not own its lifetime.

## Agent States

| State | Meaning |
|-------|---------|
| `created` | The agent exists but has not started. |
| `running` | A backend process is active. |
| `stopped` | The operator stopped the runtime. |
| `exited` | The process ended normally. |
| `failed` | Startup or runtime execution failed. |

AgentHub permits one active runtime per agent. Repeated start requests do not
create parallel processes for the same agent.

## Actions and Their Scope

### Start

Starts the configured command in the selected workspace, creates a session,
and begins persisting output.

### Interrupt or Cancel

Cancels the active ACP turn when the provider supports cancellation. Use this
when a response or tool call needs to stop without discarding the agent.

### Stop

Stops the backend process and keeps the agent definition and persisted history.
Use **Start** again when you want a new active runtime.

### Clear ACP Session

Forces the next provider interaction to create a new ACP session. This is a
provider recovery action; it does not delete AgentHub event history.

### Delete

Removes the agent and its managed session/event records. Export or back up
anything you need before deletion.

## Persisted Events

AgentHub stores agent configuration and session metadata in the main SQLite
database. High-volume output is stored in per-agent SQLite databases under:

```text
~/.agenthub/agent-events/<agent-id>.db
```

Events record `stdout`, `stderr`, `system`, or structured `acp` streams. The
history API supports session filtering and cursor-based paging, and the UI uses
that persisted history for replay after a refresh or reconnect.

Do not remove individual event database files as a recovery shortcut. The
running service may still hold connections or metadata that refer to them. Use
the product's delete action for intentional removal, or stop the service and
restore a consistent backup when repairing storage.

## Retention

History cleanup is controlled by `~/.agenthub/config.toml`:

```toml
[history]
event_retention_days = 5
vacuum_on_cleanup = false
delete_batch_size = 10000
```

- `event_retention_days = 0` disables age-based deletion.
- Cleanup deletes old records in bounded batches.
- `vacuum_on_cleanup = true` may reclaim space but adds I/O and locking work.

Choose a retention window that matches your audit and disk requirements, then
include the entire AgentHub data directory in backups if session replay is
important.

## Reconnect and Replay

1. The backend runtime continues while the browser is disconnected.
2. The browser loads persisted events when the agent view opens.
3. Live output resumes through `/sse/agents`.
4. If SSE is unavailable or stale, the UI polls the event API until streaming
   recovers.

SSE backpressure or a lagging receiver causes the server to close that stream
instead of growing memory without bound. The browser reconnects and catches up
from the database.

## Safe Recovery Order

When an agent appears stuck:

1. Check the connection badge and agent state.
2. Refresh the page to trigger history replay.
3. Inspect the server log and the configured ACP binary.
4. Interrupt the active turn, then stop/start the agent if necessary.
5. Clear only the ACP provider session when the provider thread is corrupted.
6. Restore a consistent backup before attempting any database-level repair.

See [Connection Status and Recovery](../advanced/connection-status-and-recovery.md)
for stream diagnostics.
