---
sidebar_position: 4
---

# Troubleshooting

Start with read-only evidence. Avoid deleting databases, clearing all browser
state, rotating credentials, or weakening TLS until the failure domain is
known.

## First Five Checks

```bash
agenthub --version
agenthubd --version
curl --fail http://127.0.0.1:8080/health
df -h
```

For a Debian/systemd installation:

```bash
sudo systemctl status agenthub.service --no-pager
sudo journalctl -u agenthub.service -n 200 --no-pager
```

Confirm which OS account runs the service, which `HOME` it sees, and which
`config.toml` it loads. Debian uses `/var/lib/agenthub` as `HOME`; archive and
npm installations normally use the interactive user's home.

## Server Does Not Start

Check, in order:

1. TOML syntax and file permissions.
2. Whether the configured HTTP or internal gRPC port is already in use.
3. Whether `log_path`, worktree, object, message-store, certificate, and VAPID
   parent directories are writable.
4. Whether `server.role = "node"` also has a unique `server.node_id` and
   `internal_grpc.enabled = true`.
5. The first complete error chain in the service log.

The public UI and `/health` are intentionally absent from a node-only process.

## Login Problems

- On a new instance, use **First-run setup** to create the root operator.
- Query `/api/auth/status` to distinguish an uninitialized instance from a
  password failure.
- Usernames cannot contain `@`.
- Sessions expire after 12 hours; sign in again on `invalid token`.
- For passkeys, the browser URL must match `web.rp_id` and `web.rp_origin`, and
  non-localhost deployments require HTTPS.
- Teamspace invitations create operator accounts and must belong to the same
  instance.

Do not replace `agenthub.db` merely to reset login; that discards instance
state.

## Agent Fails to Start

1. Confirm the resolved workdir exists and is writable by the service user.
2. Run the configured command as that same OS user:

   ```bash
   agenthubd --version
   ```

3. Confirm `agenthub` and `agenthubd` come from the same release.
4. Verify provider credentials in the service environment, not only your
   interactive shell.
5. For `create_worktree`, inspect repository/ref validity and the selected
   node's default worktree root.
6. If the state says `running` but the process is absent, refresh the Agents
   view so runtime reconciliation can correct stale state.

`agent is already running` means the single-runtime guard is active. Stop the
existing runtime rather than retrying starts in a loop.

## Codex Code Mode Host Is Missing

This failure means the Codex worker found `agenthubd` but could not find its
version-matched Code Mode companion beside the daemon:

```text
Code Mode is unavailable because failed to spawn code-mode host ...: host executable was not found
```

Inspect the installed layout without changing it:

```bash
daemon_path="$(command -v agenthubd)"
host_path="$(dirname "$daemon_path")/codex-code-mode-host"
test -x "$host_path"
"$host_path" --help
```

If either check fails, reinstall the complete daemon package or archive from
the same AgentHub release. For a portable archive install, copy both files from
the extracted daemon directory:

```bash
install -m 0755 /path/to/extracted-agenthubd/agenthubd "$HOME/.local/bin/agenthubd"
install -m 0755 \
  /path/to/extracted-agenthubd/codex-code-mode-host \
  "$HOME/.local/bin/codex-code-mode-host"
```

Do not substitute a companion from an unrelated Codex or AgentHub release.
The host protocol follows AgentHub's pinned Codex revision, and a mismatched
binary can fail after startup even when the path exists.

## Team Message Arrives but the Agent Does Not Reply

If a Team or `all` channel message is persisted but no agent reply appears,
check the provider log before changing mailbox state. This pair indicates an
egress failure rather than a channel fan-out failure:

```text
Falling back from WebSockets to HTTPS transport.
request timed out
```

For a service-managed daemon, configure provider egress in the AgentHub config
loaded by that service. Do not assume proxy variables exported by an
interactive shell are present in systemd:

```toml
[proxy]
http = "http://proxy.company.com:8080"
https = "http://proxy.company.com:8080"
```

Debian installations load
`/var/lib/agenthub/.agenthub/config.toml`; user-managed archive and npm
installations normally load `~/.agenthub/config.toml`. Restart the matching
service after editing the file:

```bash
sudo systemctl restart agenthub.service
systemctl --user restart agenthub.service
```

Run only the command for the service scope you actually use. A foreground
daemon must be stopped and started directly instead. After a daemon restart,
start the affected agent or Team again, then confirm the timeout does not recur
before resending messages. Persisted mailbox rows are diagnostic evidence; do
not delete or rewrite them to hide a provider connectivity failure.

## No or Stale Output

- Read the connection badge first. `SSE idle` is normal without a running
  target.
- In browser developer tools, inspect `/sse/agents`:
  - `401`: sign in again.
  - `404`: no requested agent is running.
  - gateway HTML: check the reverse proxy/upstream.
- Confirm persisted history still loads from
  `/api/agents/<agent-id>/events`.
- Refresh the page; the backend process continues and replay should fill gaps.
- Check logs for ACP exit, SSE backpressure timeout, broadcast lag, or SQLite
  errors.

Do not remove `~/.agenthub/agent-events/*.db` to reset the UI. See
[Connection Status and Recovery](../advanced/connection-status-and-recovery.md).

## History or Disk Problems

Check the data paths and configured retention:

```bash
du -sh ~/.agenthub/* 2>/dev/null
find ~/.agenthub/agent-events -type f -name '*.db' -exec du -h {} + 2>/dev/null | sort -h
```

Use `[history]` retention for routine event cleanup. `VACUUM` adds I/O and can
hold SQLite locks, so enable it deliberately. Deleting an Agent through the UI
is the supported way to remove its managed event history.

For `database is locked`, identify overlapping AgentHub processes, backup jobs,
or manual SQLite tools. Do not start a second service process against the same
data directory.

If corruption is suspected, stop the service, copy the complete data set, and
run read-only integrity checks on the copy. Restore a consistent backup rather
than rebuilding individual files in place.

## Push Notifications

- Set a valid `push.subject` and ensure `keys_path` is writable.
- Use HTTPS outside localhost and inspect service-worker registration.
- Confirm the browser permission and `/api/push/subscribe` response.
- After VAPID rotation, every browser must subscribe again.
- Treat push as best effort; verify completion in AgentHub history.

## S3/Object Storage

- Confirm the deployed release actually contains S3 support; source/default
  builds require the root `object-store-s3` feature.
- Validate bucket, endpoint, region, prefix, and the two configured credential
  environment-variable names in the service environment.
- Distinguish an S3-enabled binary from provider certification.
- For URL ingestion, check host allow/deny policy, private-network policy, size,
  redirect, timeout, retry, concurrency, and digest constraints.
- Preserve the failed object metadata and server log before retrying an upload
  whose commit status is uncertain.

## Remote Agent Nodes

- Confirm the node runs with `server.role = "node"` and matching node ID.
- Test the registered `https://` gRPC target from the main host.
- Compare CA trust and `tls_server_name` for certificate errors.
- Compare issuer, audience, shared secret, and bootstrap token for unauthorized
  errors.
- Check workdir paths on the remote filesystem.
- Synchronize clocks before investigating duplicate/stale mailbox rejection.

Do not switch to plaintext or public ingress to work around a certificate or
routing error.

## OpenAPI and Automation

- Open `/api/openapi/docs` without authentication to inspect the current
  published contract.
- Fetch `/api/openapi.json` with a bearer session that has runtime-inspect
  capability.
- Remember that the spec is incremental and does not cover every web UI route.
- Use the HTTP status and `{ "error": "..." }` body; do not parse prose as a
  stable error code.

## Escalation Bundle

Include:

- exact `agenthub` and `agenthubd` versions and install channel;
- OS/architecture and, for Linux, `ldd --version`;
- deployment mode, browser, and reverse proxy;
- sanitized config sections relevant to the failure;
- resource/agent/Team/node IDs and timestamps;
- HTTP method/status and a redacted response body;
- the matching service-log window and whether restart/replay changed behavior.

Never include bearer or SSE query tokens, passwords, provider credentials,
VAPID private keys, internal gRPC secrets, private prompts, or uploaded data.
