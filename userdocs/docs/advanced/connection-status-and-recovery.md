---
sidebar_position: 3
---

# Connection Status and Recovery

AgentHub surfaces connection health as a status badge. Use it as the first
signal before opening low-level debug output.

## Badge States

- `Online · SSE connected`: stream is healthy
- `Online · SSE connecting`: stream is opening
- `Online · SSE reconnecting`: stream is retrying after interruption
- `Online · SSE idle`: no active stream target selected
- `Offline · SSE disconnected`: browser/network is offline

## Error Banner vs Connection Badge

Connectivity issues are intentionally represented by connection status whenever
possible.

Example:

- `Connection unavailable (gateway response). Reconnecting...`

This message should be reflected by badge transitions (connecting/reconnecting)
instead of staying as a persistent error banner.

## Recovery Workflow

1. Check the connection badge first.
2. If reconnecting, wait a short period for stream recovery.
3. If still unstable, refresh the active session view.
4. If offline, restore network and re-open AgentHub.
5. If needed, open `Debug / Raw` to inspect transport events.

## When to Escalate

Escalate to backend/runtime investigation when:

- Badge oscillates between `connecting` and `reconnecting` for long periods
- Session status advances but no new output arrives
- Multiple agents show stale stream behavior simultaneously

## Related Pages

- [Troubleshooting](../operations/troubleshooting.md)
- [View Execution Output](../core/view-output.md)
