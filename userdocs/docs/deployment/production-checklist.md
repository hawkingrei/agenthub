---
sidebar_position: 2
---

# Production Checklist

Use this checklist for a shared AgentHub deployment and for every release
upgrade.

## Artifact Preflight

- Select one exact release tag and verify `SHA256SUMS.txt`.
- Install `agenthub` and `agenthub-acp` from that same release.
- Confirm both `--version` commands before rollout.
- Validate the official binary on the oldest Linux environment you intend to
  support; the project's glibc floor is not yet frozen.
- If using S3, smoke test the release artifact against the exact provider and
  bucket. An S3-enabled build alone is not provider certification.
- Keep the previous verified binaries and a configuration/data snapshot for
  rollback.

## Service and Network Baseline

- Run as a dedicated non-root OS account.
- AgentHub does not restrict workdirs itself, so scope that account's
  filesystem permissions to the repositories/workspaces it should touch.
- Keep the HTTP listener on loopback/private ingress and terminate shared
  access with an authenticated HTTPS proxy.
- Configure `web.rp_id` and `web.rp_origin` to match the public URL before
  enabling passkeys.
- Preserve SSE streaming and disable proxy buffering for `/sse/*`.
- Keep the first-run root initialization page off public unauthenticated
  ingress.

For Debian packages, the service uses `HOME=/var/lib/agenthub`. Its default
configuration and state live under `/var/lib/agenthub/.agenthub/`, and its
default workspace root is `/var/lib/agenthub/workspaces`.

## Remote Node Baseline

When Agent Nodes are enabled:

- expose node-only internal gRPC on a private `https://` endpoint;
- keep bootstrap tokens, JWT shared secrets, and TLS keys out of node registry
  metadata and logs;
- align issuer/audience/auth configuration across peers;
- prefer `mtls` when client-certificate identity is required;
- synchronize clocks and validate each registered `grpc_target` and
  `tls_server_name`;
- run one remote-agent start/output/stop smoke before admitting normal work.

## Data and Backup

The safest default is a consistent snapshot of the complete AgentHub data
directory while the service is stopped. At minimum account for:

- `agenthub.db` and its SQLite sidecar files;
- `agent-events/` per-agent event databases;
- `config.toml` and `vapid.json`;
- `message-archive/` and optional `message-bodies/` stores;
- local `objects/`, or the corresponding external object-store retention and
  recovery policy;
- internal gRPC certificates and generated auth/bootstrap files;
- worktrees only when uncommitted workspace content must be recoverable.

If any path is overridden in configuration, back up the effective path rather
than assuming it lives below `~/.agenthub`. For S3, back up control-plane
metadata and make sure bucket versioning/retention matches the database
recovery point.

Test restore into an isolated instance. A backup that has never been restored
is not release evidence.

## Upgrade Runbook

1. Stop new task submissions and let important active runs finish.
2. Stop the service.
3. Capture a consistent data/config snapshot and record both binary versions.
4. Install the new matching binary pair.
5. Start the service and verify `GET /health` returns `ok`.
6. Sign in, create/start a disposable agent, receive output, and replay it after
   a browser refresh.
7. If enabled, test browser push, OpenAPI retrieval, S3 object flow, and one
   remote Agent Node.
8. Keep the environment closed until logs and smoke checks are clean.

## Rollback

If startup or a critical smoke fails, stop the service before changing files.
Restore the previous binaries and configuration. Restore the pre-upgrade data
snapshot when the newer process may have migrated or mutated persistent state;
do not run an older binary against a possibly upgraded database by assumption.

Record the failed version, exact artifact checksums, first failing step, and
matching logs before reopening the previous version.

## Related Pages

- [Installation and Startup](../getting-started/installation.md)
- [Deployment Overview and Topology](./overview-and-topology.md)
- [Remote Node Transport](./remote-node-transport.md)
- [Security and Path Safety](../operations/security-and-path-safety.md)
