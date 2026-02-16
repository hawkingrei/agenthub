---
sidebar_position: 2
---

# Production Checklist

Use this checklist before and after deploying AgentHub in a shared environment.

## Preflight Checklist

- Define explicit `safe_paths` for each team/project root.
- Validate `worktree.default_root` exists and is writable.
- Confirm runtime user permissions for repository paths.
- Confirm Node/Rust build artifacts are reproducible in CI.
- Confirm rollback plan (previous binary + config snapshot).

## Security Baseline

- Run AgentHub as a non-root user.
- Keep network exposure minimal (private network + reverse proxy).
- Use TLS at the edge.
- Enforce strong user credentials and periodic rotation.
- Avoid putting sensitive directories under `safe_paths`.

## Data and Backup

AgentHub persists runtime data under `~/.agenthub/` by default.

Minimum backup targets:

- `~/.agenthub/agenthub.db`
- deployment `config.toml`

Recommended cadence:

- Daily snapshot for active environments
- Extra snapshot before each upgrade

## Upgrade Runbook

1. Stop new task submissions.
2. Backup `agenthub.db` and `config.toml`.
3. Deploy new binary/config.
4. Restart service.
5. Run smoke checks:
   - login
   - create/start one test agent
   - session replay after refresh
6. Monitor logs for startup/runtime errors.

## Failure Rollback Rule

If smoke checks fail or critical flows regress:

1. Restore previous binary.
2. Restore previous config.
3. Restore database snapshot when schema compatibility is uncertain.
4. Re-run smoke checks before reopening service.

## Related Pages

- [Deployment Overview and Topology](./overview-and-topology.md)
- [Security and Path Safety](../operations/security-and-path-safety.md)
