---
sidebar_position: 6
---

# Session Lifecycle

This page explains how session state changes over time, how ACP (Agent Control Protocol) events are captured, and what each action means for users.

## Session States

An agent session transitions through the following states:

| State | Description |
|-------|-------------|
| `created` | Agent exists but process is not running yet |
| `running` | Task process is actively executing |
| `completed` | Task finished successfully |
| `failed` | Task encountered an error |
| `cancelled` | Task was manually stopped by user |
| `exited` | Process exited (may be expected or unexpected) |

## User Actions and Effects

### Start

- Boots the runtime process for the selected agent
- Reuses single active runtime guard to prevent duplicate starts
- Initializes ACP session with the configured provider
- Begins capturing events to persistent storage

### Stop/Interrupt

- Requests the in-progress run to stop gracefully
- Sends termination signal to the agent process
- Keeps all history for later audit and replay
- Session transitions to `cancelled` state

### Delete

- Removes the agent entry from management list
- Optionally removes associated event history (configurable retention)
- Should be used only after output/history are no longer needed

## ACP Event Model

AgentHub uses the **Agent Control Protocol (ACP)** to capture structured events from agent runs. Unlike raw terminal output, ACP events preserve context and enable rich replay.

### Event Streams

Events are captured in two parallel streams:

- **`acp`**: Structured ACP protocol messages (conversation, tool calls, plans)
- **`system`**: System-level output (process stdout/stderr, initialization logs)

### Event Storage

Each agent has a dedicated SQLite database for events:

```
~/.agenthub/agent-events/{agent_id}.db
```

The `agent_events` table schema:

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Auto-increment primary key |
| `session_id` | TEXT | ACP session identifier |
| `seq` | TEXT | Sequence UUID (v7) for ordering |
| `ts` | INTEGER | Unix timestamp (seconds) |
| `stream` | TEXT | Stream type: `acp` or `system` |
| `message` | BLOB | Event payload (JSON bytes) |

### Event Types

Common ACP event types captured:

```json
// Conversation message
{
  "type": "conversation",
  "role": "assistant",
  "content": "I'll help you with that..."
}

// Tool call
{
  "type": "tool_call",
  "tool": "shell",
  "input": { "command": "ls -la" },
  "output": "..."
}

// Plan/reasoning
{
  "type": "plan",
  "text": "1. First I'll check the files..."
}

// Debug information
{
  "type": "debug",
  "level": "info",
  "message": "Initializing context..."
}
```

### Event Retention and Cleanup

AgentHub automatically manages event storage:

**Configuration** (`~/.agenthub/config.toml`):

```toml
[history]
# Days to retain events (default: 5)
event_retention_days = 5

# Run VACUUM after cleanup (default: false)
vacuum_on_cleanup = false
```

**Cleanup Behavior**:

- Runs during idle periods (no activity for configured timeout)
- Processes deletion in batches to minimize impact
- Removes events older than `event_retention_days`
- Optionally runs `VACUUM` to reclaim disk space

**Manual Cleanup**:

If you need to free space immediately:

```bash
# Stop the agent first
# Then remove the agent's event database
rm ~/.agenthub/agent-events/{agent_id}.db
```

## Reconnect Behavior

A key feature of AgentHub is **session persistence** across browser disconnects:

1. **During Disconnect**:
   - Backend process continues running
   - Events continue to be captured and stored
   - Agent state is maintained

2. **On Reconnect**:
   - UI retrieves session status from backend
   - Event history is replayed from storage
   - Live updates resume via SSE (Server-Sent Events)

3. **Multi-Tab Support**:
   - Multiple browser tabs can view the same session
   - All tabs receive live updates
   - Changes are synchronized across clients

## History Replay

When you reopen a session, AgentHub replays stored events to reconstruct the view:

1. Fetches events from the agent's SQLite database
2. Renders ACP events as structured timeline
3. Shows system output in raw/stream view
4. Maintains chronological ordering via sequence UUIDs

### Performance Considerations

- Each agent has its own database to avoid contention
- Indexes on `(session_id, seq)` and `(ts)` for efficient queries
- BLOB storage for message content to handle large payloads
- Lazy loading for long sessions (loads recent events first)

## Operational Tips

- **One goal per session**: Keep history understandable by focusing each session on a specific task
- **New sessions for experiments**: Use fresh sessions for risky refactors to isolate potential issues
- **Keep failed sessions**: Don't delete immediately; failed sessions contain valuable debugging context
- **Monitor disk usage**: Event databases can grow; adjust retention settings based on your storage
- **Backup considerations**: Include `~/.agenthub/agent-events/` in backups if history is critical

## Troubleshooting

### Session Shows "Running" But No Output

1. Check browser console for SSE connection errors
2. Verify `~/.agenthub/agent-events/{agent_id}.db` exists and is writable
3. Try refreshing the page to trigger history replay
4. Check server logs for ACP event sink errors

### History Replay Is Slow

- Large sessions (>10k events) may take time to load
- Consider reducing `event_retention_days` for high-volume agents
- Event databases are per-agent; deleting old agents frees space

### Events Missing From History

- Events are persisted asynchronously; very recent events may not appear immediately
- If using `vacuum_on_cleanup = true`, cleanup runs may briefly lock the database
- Check server logs for SQLite errors
