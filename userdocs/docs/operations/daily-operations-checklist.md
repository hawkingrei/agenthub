---
sidebar_position: 1
---

# Daily Operations Checklist

## Before Admitting Work

- `curl --fail http://127.0.0.1:8080/health` returns `ok`.
- The deployed `agenthub` and `agenthub-acp` versions match.
- Disk space is healthy for the data directory, worktrees, and local objects.
- Required remote nodes show a recent last-seen signal.
- The selected repository/workdir is inside an effective `safe_paths` root.
- No unresolved backup, migration, or provider incident is in progress.

## During Execution

- Keep one bounded objective per agent or Team task.
- Watch both agent status and the connection badge; an SSE reconnect alone does
  not mean the process stopped.
- Review permission requests before allowing mutating tools.
- Use **Interrupt** for a drifting turn and **Stop** for the runtime; do not
  delete an agent as a recovery shortcut.
- Check service logs when repeated starts, provider crashes, or stream recovery
  failures occur.

## Before Accepting a Result

- Review the changed files and workspace rather than relying on the final
  message alone.
- Run the relevant tests or build commands in the same workspace.
- Confirm no unrelated secret, artifact, or worktree content was added.
- Preserve the session or Team task link needed for audit/review.

## End of Day

- Stop runtimes that should not continue unattended.
- Keep failed sessions until useful diagnostics are captured.
- Remove obsolete agents/worktrees only through deliberate product and Git
  cleanup flows.
- Review warnings for SQLite, object storage, push, internal gRPC, and provider
  exits.

## Scheduled Maintenance

- Snapshot and restore-test the complete data set.
- Review `safe_paths`, users/roles, devices, remote nodes, and TLS expiry.
- Check event retention and disk growth before enabling SQLite `VACUUM`.
- Re-run the release artifact smoke after upgrades, including S3 or remote-node
  paths that your deployment advertises.
- Patch AgentHub, ACP providers, browsers, and host dependencies.
