---
sidebar_position: 4
---

# Troubleshooting

This page helps you diagnose and resolve common AgentHub issues.

## Quick Diagnostic Commands

```bash
# Check if AgentHub is running
curl -i http://localhost:8080/

# Check authentication
curl -H "Authorization: Bearer $TOKEN" http://localhost:8080/api/me

# Verify internal gRPC (if enabled)
agenthub actor inbox --actor-id test --limit 1

# Check disk space for event databases
df -h ~/.agenthub

# View recent logs
journalctl -u agenthub -n 100 --no-pager
```

## Login Issues

### Cannot Access Login Page

**Symptoms**: Browser cannot connect to `http://localhost:8080`

**Diagnostic Steps**:

1. Check if AgentHub is running:
   ```bash
   pgrep -a agenthub
   ```

2. Verify the server is listening:
   ```bash
   netstat -tlnp | grep 8080
   # or
   ss -tlnp | grep 8080
   ```

3. Check config for correct listen address:
   ```toml
   [server]
   listen = "127.0.0.1:8080"  # or "0.0.0.0:8080" for remote access
   ```

**Solutions**:

- Start AgentHub: `agenthub`
- Check firewall rules if accessing remotely
- Verify port is not in use by another process

### Invalid Credentials

**Symptoms**: "Invalid username or password" error

**Solutions**:

- Verify caps lock is off
- Check if root user exists: check `~/.agenthub/agenthub.db` with sqlite3
- If root password lost, you may need to reset the database (data loss)

### Join Token Issues

**Symptoms**: "Invalid join token" or "Join token expired"

**Solutions**:

- Generate new join challenge via admin API
- Complete join within token expiration window
- Verify PIN is entered correctly

## Agent Startup Issues

### Agent Cannot Start

**Symptoms**: Clicking Start shows error or no response

**Common Causes**:

| Cause | Diagnostic | Solution |
|-------|------------|----------|
| Invalid workdir | Check path exists | Create directory or choose different path |
| Path outside safe_paths | Check `safe_paths` config | Add path to config or use allowed path |
| Missing ACP binary | `which agenthub-codex-acp` | Install the bundled ACP adapter or set `codex_acp.binary` |
| Permission denied | Check directory permissions | `chmod 755 /path/to/workdir` |

### "Workdir path must be within allowed safe paths"

**Solutions**:

1. Add path to `safe_paths` in config:
   ```toml
   safe_paths = [
     "/home/you/projects",
     "/new/path/here",
   ]
   ```

2. Use `create_worktree` mode (uses configured `default_root`)

3. Restart AgentHub after config changes

### ACP Binary Starts But Fails Immediately

**Symptoms**: Agent starts and exits quickly, or the server logs show ACP
handshake / startup errors.

**Diagnostic Steps**:

1. Check which binary AgentHub is launching:
   ```bash
   which agenthub-codex-acp
   ```
2. Verify the adapter binary is the expected one:
   ```bash
   agenthub-codex-acp --version
   ```
3. Compare it to the repository baseline:
   - Rust `1.95.0`
   - official Codex `0.121.x`

**Solutions**:

- Rebuild from the current repository state if the binary is stale
- Avoid mixing an older fork-pinned ACP binary into a newer AgentHub deployment
- If you intentionally use a custom ACP binary, set `codex_acp.binary` to that
  exact path and verify protocol compatibility first

### "Agent is already running"

**Solutions**:

- Wait for current run to complete
- Stop the agent first, then restart
- Check if zombie process exists:
  ```bash
  ps aux | grep agenthub-codex-acp
  ```

## Session and Output Issues

### No Output or Stale Output

**Symptoms**: Agent shows "running" but no new output appears

**Diagnostic Steps**:

1. Check connection badge in UI header
   - `connected`: SSE connection active
   - `connecting` / `reconnecting`: Connection issues

2. Check browser console for SSE errors:
   ```javascript
   // In browser console
   new EventSource('/api/agents/{agent_id}/events')
   ```

3. Verify event database is writable:
   ```bash
   ls -la ~/.agenthub/agent-events/{agent_id}.db
   ```

4. Check server logs for ACP event sink errors

**Recovery Actions**:

1. Keep the current session for evidence
2. Create a fresh session with same prompt
3. Compare behavior to isolate differences
4. Check [Connection Status and Recovery](../advanced/connection-status-and-recovery.md)

### History Replay Is Slow

**Symptoms**: Opening a completed session takes long time to load

**Solutions**:

- Large sessions (>10k events) naturally take time
- Reduce `event_retention_days` in config
- Delete old agent event databases:
  ```bash
  rm ~/.agenthub/agent-events/{old_agent_id}.db
  ```

### Events Missing From History

**Symptoms**: Recent events don't appear in session view

**Causes**:

- Events persisted asynchronously (small delay normal)
- Database locked during cleanup (if `vacuum_on_cleanup = true`)
- Event database corruption

**Solutions**:

- Wait a few seconds and refresh
- Check server logs for SQLite errors
- Restart AgentHub if database appears stuck

## Team Issues

### Team Runtime Won't Start

**Symptoms**: "Start Team" fails or hangs

**Diagnostic Steps**:

1. Check Team spec is valid JSON
2. Verify all `member_id` references are valid
3. Ensure `leader_member_id` references existing member
4. Check `entrypoint` is defined

**Common Errors**:

| Error | Cause | Solution |
|-------|-------|----------|
| `spec.members must be an array` | Invalid spec format | Ensure members is JSON array |
| `spec.leader_member_id must reference a defined member` | Leader not in members list | Add leader to members or fix reference |
| `step already exists for run` | Duplicate step key | Use unique step keys |

### Permission Review Not Routing

**Symptoms**: Tool permission requests not reaching reviewers

**Solutions**:

- Check Team has leader defined
- Verify `requester_role` is set correctly
- Ensure `agenthub actor ...` commands work:
  ```bash
  agenthub actor team-members --actor-id <leader_actor_id>
  ```

### Team Messages Not Delivered

**Symptoms**: Messages sent but not received by members

**Diagnostic Steps**:

1. Check actor inbox:
   ```bash
   agenthub actor inbox --actor-id <member_id> --run-id <run_id>
   ```

2. Verify mailbox routing is correct
3. Check internal gRPC connectivity

## Internal gRPC and Actor Issues

### "internal grpc client not available"

**Solutions**:

1. Enable internal gRPC in config:
   ```toml
   [internal_grpc]
   enabled = true
   listen = "127.0.0.1:50051"
   ```

2. Restart AgentHub
3. Verify `shared_secret` is explicitly configured

### Actor CLI Commands Fail

**Symptoms**: `agenthub actor inbox` returns error

**Diagnostic Steps**:

1. Verify internal gRPC is enabled
2. Check `shared_secret` matches between config and CLI context
3. Ensure authority process is running
4. Verify `--actor-id` is correct

**Solutions**:

```bash
# Test basic connectivity
agenthub actor team-members --actor-id <id>

# Check inbox with explicit run scope
agenthub actor inbox --actor-id <id> --run-id <run_id> --limit 10
```

## Agent Node Issues

### Cannot Register Remote Node

**Symptoms**: "failed to connect to remote node" error

**Diagnostic Steps**:

1. Verify remote AgentHub is running with internal gRPC enabled
2. Check network connectivity:
   ```bash
   telnet remote-host 50051
   ```
3. Verify TLS certificates are valid
4. Check firewall rules

### Remote Agent Won't Start

**Symptoms**: Agent assigned to remote node but doesn't start

**Solutions**:

- Verify node is reachable from main control plane
- Check node's `Default worktree root` is configured or provide explicit `Workdir`
- Review node logs on remote machine
- Ensure same `shared_secret` across cluster

## Performance Issues

### High CPU Usage

**Diagnostic Steps**:

1. Check active agent count
2. Review event database sizes:
   ```bash
   du -sh ~/.agenthub/agent-events/*.db
   ```
3. Monitor cleanup operations

**Solutions**:

- Reduce `event_retention_days`
- Lower `delete_batch_size` for gentler cleanup
- Delete old/unused agents

### High Disk Usage

**Diagnostic Steps**:

```bash
# Check AgentHub data directory
 du -sh ~/.agenthub/*

# Find largest event databases
ls -lhS ~/.agenthub/agent-events/*.db | head -10
```

**Solutions**:

- Enable `vacuum_on_cleanup = true`
- Manually delete old agent databases
- Reduce retention period

### Slow Query Performance

**Causes**:

- Large event databases without cleanup
- Missing indexes (should be automatic)
- Concurrent cleanup operations

**Solutions**:

- Regular cleanup via `event_retention_days`
- Schedule maintenance window for VACUUM
- Monitor with: `sqlite3 agent.db "PRAGMA integrity_check;"`

## Notification Issues

### Push Notifications Not Received

**Diagnostic Steps**:

1. Check browser notification permission
2. Verify VAPID keys exist:
   ```bash
   cat ~/.agenthub/vapid.json
   ```
3. Check subscription status in UI
4. Verify `subject` is configured

**Solutions**:

- Re-subscribe in browser
- Rotate VAPID keys if corrupted
- Check HTTPS requirement for production

## Database Issues

### SQLite Lock/Timeout Errors

**Symptoms**: "database is locked" errors in logs

**Solutions**:

- Reduce concurrent operations
- Check for long-running queries
- Ensure proper connection pooling

### Database Corruption

**Symptoms**: SQLite integrity check failures

**Recovery**:

```bash
# Backup first
cp ~/.agenthub/agenthub.db ~/.agenthub/agenthub.db.backup

# Check integrity
sqlite3 ~/.agenthub/agenthub.db "PRAGMA integrity_check;"

# For event databases
sqlite3 ~/.agenthub/agent-events/{agent_id}.db "PRAGMA integrity_check;"
```

## Getting Help

When reporting issues, include:

1. **AgentHub version**: `agenthub --version`
2. **Config** (sanitized): `cat ~/.agenthub/config.toml`
3. **Logs** at debug level:
   ```toml
   # Add to config
   [logging]
   level = "debug"
   ```
4. **System info**:
   ```bash
   uname -a
   df -h
   free -h
   ```
5. **Reproduction steps**

## Recovery Strategy

For serious issues:

1. **Keep evidence**: Don't delete failed sessions
2. **Create fresh environment**: New workdir/worktree
3. **Isolate variables**: Test with minimal config
4. **Incremental changes**: Add complexity gradually
5. **Monitor**: Watch logs during recovery

See also:
- [Daily Operations Checklist](./daily-operations-checklist.md)
- [Security and Path Safety](./security-and-path-safety.md)
- [API Error Reference](./api-error-reference.md)
