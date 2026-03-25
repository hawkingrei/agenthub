---
sidebar_position: 3
---

# Connection Status and Recovery

AgentHub uses Server-Sent Events (SSE) for real-time updates. This guide explains the connection lifecycle, recovery mechanisms, and troubleshooting techniques.

## Architecture Overview

```
┌──────────────┐      HTTP/SSE       ┌──────────────────┐
│   Browser    │ ◄──────────────────► │  AgentHub Server │
│   (UI)       │   Event Stream      │                  │
└──────────────┘                     └────────┬─────────┘
                                              │
                    ┌─────────────────────────┼─────────────────────────┐
                    │                         │                         │
                    ▼                         ▼                         ▼
            ┌──────────────┐         ┌──────────────┐         ┌──────────────┐
            │ Agent Process│         │ Agent Process│         │ Agent Process│
            │  (running)   │         │  (running)   │         │  (running)   │
            └──────────────┘         └──────────────┘         └──────────────┘
```

**Key Points**:
- Each agent session has its own SSE stream
- Server pushes events as they occur
- Client automatically reconnects on interruption
- Events are persisted server-side for replay

## Connection Badge States

The connection badge in the UI header shows stream health:

| State | Badge | Meaning | Action Needed |
|-------|-------|---------|---------------|
| Connected | `Online · SSE connected` | Stream healthy | None |
| Connecting | `Online · SSE connecting` | Opening connection | Wait |
| Reconnecting | `Online · SSE reconnecting` | Retry after interruption | Wait |
| Idle | `Online · SSE idle` | No agent selected | Select agent |
| Disconnected | `Offline · SSE disconnected` | Network offline | Check network |

### State Transitions

```
         ┌─────────────┐
         │    Idle     │
         └──────┬──────┘
                │ Select Agent
                ▼
         ┌─────────────┐
         │  Connecting │
         └──────┬──────┘
           ┌────┴────┐
           │         │
           ▼         │
    ┌─────────────┐  │
    │  Connected  │  │ Error
    └──────┬──────┘  │
           │         │
    Network│         │
    Issue  │         │
           ▼         │
    ┌─────────────┐  │
    │ Reconnecting│◄─┘
    └──────┬──────┘
           │ Success
           ▼
    ┌─────────────┐
    │  Connected  │
    └─────────────┘
```

## Server-Sent Events (SSE) Deep Dive

### How SSE Works

```javascript
// Browser connects to SSE endpoint
const eventSource = new EventSource('/api/agents/{agent_id}/events')

eventSource.onmessage = (event) => {
  const data = JSON.parse(event.data)
  // Handle agent output, status changes, etc.
}

eventSource.onerror = (error) => {
  // Connection interrupted, auto-reconnect triggered
}
```

### Event Types

| Event Type | Description |
|------------|-------------|
| `output` | New agent output (stdout/stderr) |
| `status` | Agent status change (running → completed) |
| `acp_event` | Structured ACP protocol event |
| `error` | Agent or transport error |
| `heartbeat` | Keepalive ping (every 30s) |

### Heartbeat Mechanism

```
Server sends:  {"type": "heartbeat", "ts": 1710000000}
Every 30 seconds
```

**Purpose**:
- Detect half-open connections
- Keep proxy timeouts at bay
- Validate client is still listening

## Automatic Recovery

### Reconnection Strategy

AgentHub implements exponential backoff for reconnection:

```
Attempt 1: Wait 1 second
Attempt 2: Wait 2 seconds
Attempt 3: Wait 4 seconds
Attempt 4: Wait 8 seconds
Attempt 5+: Wait 30 seconds (max)
```

### Event Replay on Reconnect

When reconnecting, the server:

1. Accepts new SSE connection
2. Sends missed events since last known sequence
3. Continues with live events

```javascript
// Client tracks last received event
const lastEventId = event.lastEventId

// On reconnect, server sends:
// Events with id > lastEventId
```

### Recovery Checklist

1. **Wait briefly** (up to 30 seconds)
2. **Check badge state** - should show `reconnecting`
3. **Observe output** - missed events appear when connected
4. **Manual refresh** if stuck in `reconnecting` > 60s

## Common Scenarios

### Scenario 1: Brief Network Interruption

**Symptoms**:
- Badge shows `reconnecting` for 5-10 seconds
- Output resumes automatically
- No data loss

**Recovery**:
1. Wait for automatic reconnection
2. Verify output resumes
3. No action needed

### Scenario 2: Extended Network Outage

**Symptoms**:
- Badge shows `disconnected`
- UI becomes unresponsive
- Network icon shows offline

**Recovery**:
1. Restore network connection
2. Refresh browser page
3. Reconnect to session
4. History replay shows all missed events

### Scenario 3: Server Restart

**Symptoms**:
- All agents show `reconnecting` simultaneously
- Badge stuck in connecting state
- Cannot access other UI features

**Recovery**:
1. Wait for server to restart
2. Refresh page after server is back
3. All sessions recoverable from history

### Scenario 4: Agent Process Crash

**Symptoms**:
- Status changes to `failed`
- SSE connection remains active
- No new output

**Recovery**:
1. Check agent logs in `Debug / Raw` tab
2. Restart agent if needed
3. Create new session if unrecoverable

## Manual Recovery Steps

### Step 1: Check Connection Badge

```
Online · SSE connected    → Stream healthy
Online · SSE reconnecting → Wait for auto-recovery
Offline · SSE disconnected → Check network
```

### Step 2: Verify Network

```bash
# Test connectivity
curl -I http://localhost:8080/

# Check SSE endpoint
curl -N http://localhost:8080/api/agents/{agent_id}/events \
  -H "Authorization: Bearer $TOKEN"
```

### Step 3: Refresh Session View

1. Navigate away from agent view
2. Return to agent view
3. New SSE connection established
4. History replay begins

### Step 4: Clear Browser Cache (Last Resort)

```
1. Open DevTools (F12)
2. Application → Clear Storage
3. Clear site data
4. Refresh page
5. Login again
```

## Debug Output

### Raw Event Stream

Access raw SSE stream for debugging:

```bash
# Terminal 1: Start agent
curl -X POST http://localhost:8080/api/agents/{id}/start \
  -H "Authorization: Bearer $TOKEN"

# Terminal 2: Monitor events
curl -N http://localhost:8080/api/agents/{id}/events \
  -H "Authorization: Bearer $TOKEN" | jq .
```

### Browser DevTools

**Network Tab**:
```
1. Open DevTools (F12)
2. Network tab
3. Filter by "events"
4. Observe SSE connection
```

**Console Tab**:
```javascript
// Check EventSource state
console.log(eventSource.readyState)
// 0 = CONNECTING, 1 = OPEN, 2 = CLOSED

// Manual reconnect
eventSource.close()
location.reload()
```

### Server Logs

```bash
# Follow server logs
journalctl -u agenthub -f

# Or if running directly
agenthub 2>&1 | grep -i sse
```

## Advanced Recovery Techniques

### Forcing Reconnect

```javascript
// In browser console
window.location.reload()

// Or more targeted
const agentId = 'your-agent-id'
fetch(`/api/agents/${agentId}/events`, {method: 'HEAD'})
  .then(() => console.log('Server reachable'))
  .catch(() => console.log('Server unreachable'))
```

### Event Sequence Debugging

```bash
# Check event sequence in database
sqlite3 ~/.agenthub/agent-events/{agent_id}.db \
  "SELECT seq, ts, stream FROM agent_events ORDER BY id DESC LIMIT 20;"
```

### Proxy Timeout Issues

If behind a reverse proxy:

```nginx
# nginx.conf
location /api/agents/ {
    proxy_pass http://agenthub;
    proxy_http_version 1.1;
    
    # Critical for SSE
    proxy_set_header Connection '';
    proxy_set_header Cache-Control 'no-cache';
    
    # Extend timeouts
    proxy_read_timeout 86400s;
    proxy_send_timeout 86400s;
}
```

## Monitoring and Alerting

### Key Metrics

| Metric | Good | Warning | Critical |
|--------|------|---------|----------|
| SSE connection rate | > 95% | 90-95% | < 90% |
| Reconnection count | 0-1/min | 2-5/min | > 5/min |
| Event latency | < 100ms | 100-500ms | > 500ms |
| Error rate | < 0.1% | 0.1-1% | > 1% |

### Prometheus Metrics

```python
from prometheus_client import Counter, Gauge

sse_connections_total = Counter('agenthub_sse_connections_total', 'Total SSE connections')
sse_active_connections = Gauge('agenthub_sse_active_connections', 'Active SSE connections')
sse_reconnections_total = Counter('agenthub_sse_reconnections_total', 'Total reconnections')
```

### Alerting Rules

```yaml
# Example alerting
rules:
  - alert: HighReconnectionRate
    expr: rate(agenthub_sse_reconnections_total[5m]) > 0.1
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "High SSE reconnection rate"
      
  - alert: NoActiveConnections
    expr: agenthub_sse_active_connections == 0
    for: 1m
    labels:
      severity: critical
```

## Troubleshooting Matrix

| Symptom | Likely Cause | Solution |
|---------|-------------|----------|
| Constant reconnecting | Proxy timeout | Increase proxy timeouts |
| No output after reconnect | Event ordering issue | Refresh page |
| SSE 401 errors | Token expired | Re-login |
| SSE 404 errors | Agent deleted | Check agent exists |
| Slow event delivery | Server overload | Check server resources |
| Connection drops every 30s | Load balancer timeout | Configure LB for long connections |

## Best Practices

### For Users

1. **Don't panic on brief disconnections** - Auto-recovery works
2. **Keep sessions open during important runs** - For visual monitoring
3. **Refresh if stuck reconnecting > 60s** - Forces fresh connection
4. **Check Debug/Raw for details** - When issues persist

### For Operators

1. **Configure proper timeouts** - On all proxies/load balancers
2. **Monitor reconnection rates** - Early warning of issues
3. **Scale horizontally** - If SSE connections overwhelm server
4. **Use HTTP/2** - Better connection handling

### For Developers

1. **Implement proper error handling** - In custom clients
2. **Handle reconnect gracefully** - Replay from last known event
3. **Test network interruptions** - During development
4. **Log connection events** - For debugging

## When to Escalate

Escalate to backend/runtime investigation when:

- Badge oscillates between `connecting` and `reconnecting` for > 5 minutes
- Session status advances but no new output arrives (possible event loss)
- Multiple agents show stale stream behavior simultaneously
- Server logs show SSE errors or panics
- Event replay shows gaps in sequence numbers

## Related Pages

- [Troubleshooting](../operations/troubleshooting.md)
- [View Execution Output](../core/view-output.md)
- [Session Lifecycle](../core/session-lifecycle.md)
