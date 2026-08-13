---
sidebar_position: 3
---

# Connection Status and Recovery

AgentHub uses two complementary paths for session output:

- authenticated REST requests load persisted event history;
- Server-Sent Events (SSE) deliver new output while an agent is running.

The persisted event API is the recovery source of truth. A temporary stream
failure should delay live rendering, not erase completed output.

## Connection Badge States

| Badge | Meaning | Recommended action |
|-------|---------|--------------------|
| `Online · SSE connected` | Browser and live stream are healthy. | None. |
| `Online · SSE connecting` | A stream target exists and the first connection is opening. | Wait briefly. |
| `Online · SSE reconnecting` | The prior stream failed or became stale. | Check proxy/server health if it persists. |
| `Online · SSE idle` | Network is online but no running agent needs a stream. | Start or select a running agent if expected. |
| `Offline · SSE disconnected` | The browser reports that the network is offline. | Restore connectivity. |

## Current Endpoints

The web UI opens the multi-agent stream:

```text
GET /sse/agents?ids=<comma-separated-agent-ids>&token=<session-token>
```

A single-agent stream also exists:

```text
GET /sse/agents/<agent-id>?token=<session-token>
```

SSE uses a query token because the browser `EventSource` API cannot attach the
normal bearer header. Treat browser history, proxy logs, and copied URLs as
sensitive. Do not paste a live stream URL into tickets or chat.

Persisted history uses the authenticated REST route:

```text
GET /api/agents/<agent-id>/events?limit=20&before_id=<event-id>
```

Optional `session_id` filters history to one session. This REST endpoint
returns JSON; it is not the SSE stream.

## Stream Behavior

- The server emits a heartbeat data message every 15 seconds.
- Output may arrive as one `output` or `acp` message, or as a `batch`.
- The server bounds each browser stream. Sustained backpressure or receiver lag
  closes the connection so replay can recover without unbounded buffering.
- The web UI reconnects with exponential delays capped at 30 seconds.
- While the stream is unavailable or stale, the UI polls persisted events and
  merges unseen records by event identity.

Reverse proxies must preserve `text/event-stream`, avoid response buffering,
and allow connections to remain open beyond the heartbeat interval. The server
sets `Cache-Control: no-cache` and `X-Accel-Buffering: no` on SSE responses.

## Recovery Checklist

1. Read the exact connection badge; `idle` is not a failure.
2. Confirm the server health endpoint:

   ```bash
   curl --fail http://127.0.0.1:8080/health
   ```

3. In browser developer tools, inspect the request to `/sse/agents`:
   - `401` means the session expired; sign in again.
   - `404` means none of the requested agents is currently running.
   - a gateway HTML response indicates proxy/upstream failure.
4. Confirm normal authenticated REST requests still work. If history loads but
   SSE does not, focus on proxy buffering, idle timeouts, and routing.
5. Refresh the page. The backend task continues and history replay should fill
   the gap.
6. Inspect service logs for `backpressure timeout`, `broadcast lagged`, or an
   ACP process failure.

## Manual SSE Check

Copy a short-lived session token from an authenticated development environment
only. Avoid shell history when possible.

```bash
curl -N \
  "http://127.0.0.1:8080/sse/agents/<agent-id>?token=<session-token>"
```

You should see `heartbeat` messages and JSON output while the agent runs. Stop
the command after diagnosis and revoke the session if the URL was exposed.

## What Not to Do

- Do not delete per-agent databases to repair a live stream.
- Do not clear all browser storage before checking the status code and server
  logs; that destroys useful session evidence.
- Do not build alerts around undocumented `/metrics` endpoints. AgentHub does
  not currently expose a Prometheus contract.
- Do not assume an SSE disconnect stopped the agent process.

## Escalation Evidence

Capture the AgentHub version, browser, proxy, agent ID, badge state, HTTP status
for `/sse/agents`, whether REST history still loads, and the matching server log
window. Redact query tokens and credentials.
