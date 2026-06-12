---
sidebar_position: 2
---

# Production Checklist

Use this checklist before and after deploying AgentHub in a shared environment.

## Preflight Checklist

- Define explicit `safe_paths` for each team/project root.
- Validate `worktree.default_root` exists and is writable.
- For Debian package installs, update `/var/lib/agenthub/.agenthub/config.toml` and grant the
  `agenthub` service user access to each configured workspace root.
- Confirm runtime user permissions for repository paths.
- Confirm Node/Rust build artifacts are reproducible in CI.
- Confirm rollback plan (previous binary + config snapshot).

## Security Baseline

- Run AgentHub as a non-root user.
- Keep network exposure minimal (private network + reverse proxy).
- Use TLS at the edge.
- Enforce strong user credentials and periodic rotation.
- Avoid putting sensitive directories under `safe_paths`.

## Remote Node Baseline

When remote Agent Nodes are enabled:

- Keep node-only processes on internal `https://` gRPC endpoints.
- Register remote nodes from the main control plane; do not treat registry rows
  as credential storage.
- Keep internal gRPC `issuer`, `audience`, and `shared_secret` aligned across
  peers until per-node identity is available.
- Keep node clocks synchronized so timestamp-window validation is reliable.
- Smoke test duplicate relay retries in staging and confirm they do not create
  duplicate destination mailbox rows.
- Keep the dedicated gRPC port until same-port HTTP plus gRPC multiplexing has
  been validated with your reverse proxy.

## Data and Backup

AgentHub persists runtime data under `~/.agenthub/` by default.

For Debian package installs, the default service sets `HOME=/var/lib/agenthub`, so the persisted
runtime data is under `/var/lib/agenthub/.agenthub/`.

Minimum backup targets:

- `~/.agenthub/agenthub.db` for source, Homebrew, and archive installs.
- `/var/lib/agenthub/.agenthub/agenthub.db` for Debian package installs.
- deployment `config.toml`.

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
- [Remote Node Transport](./remote-node-transport.md)
- [Security and Path Safety](../operations/security-and-path-safety.md)
