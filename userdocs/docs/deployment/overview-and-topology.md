---
sidebar_position: 1
---

# Deployment Overview and Topology

This page describes a practical deployment model for running AgentHub as a
single service.

## Architecture at a Glance

AgentHub deployment is intentionally simple:

- One backend process (Rust)
- One embedded web UI served by that same process
- One SQLite database for persisted state
- Browser clients connected with HTTP + SSE

Optional scale-out deployments can add remote Agent Nodes for execution and
mailbox delivery while keeping AgentHub as the main control plane.

## Runtime Components

Prepare these components before rollout:

1. AgentHub executable (or `cargo run` for development)
2. A valid `config.toml`
3. Writable runtime home (default under `~/.agenthub/`)
4. Explicit `safe_paths` for all allowed repositories/workdirs

## Deployment Modes

### Local development mode

Use this for daily personal work:

```bash
cd web
npm install
npm run build
cd ..
cargo run -- -c /path/to/config.toml
```

### Team internal mode

Use this for shared environments:

- Build and run a fixed binary
- Run under a dedicated OS user (not root)
- Manage process lifecycle with a supervisor (for example systemd)
- Keep AgentHub behind an internal reverse proxy or VPN boundary

### Distributed node mode

Use this when execution must span multiple machines:

- Keep one AgentHub instance as the main control plane
- Register remote Agent Nodes from the `Agents` page
- Use encrypted gRPC between AgentHub and nodes
- Configure a node-specific default worktree root when remote filesystems differ

#### Distributed node prerequisites

Every participating node still runs the same `agenthub` binary. The difference
is which node acts as the main control plane and which nodes are registered as
remote execution targets.

Recommended shared baseline on every node:

```toml
[internal_grpc]
enabled = true
listen = "0.0.0.0:50051"

[internal_grpc.security]
mode = "tls" # tls | mtls | disabled
cert_dir = "~/.agenthub/internal-grpc"

[internal_grpc.auth]
issuer = "agenthub"
audience = "agenthub-internal"
# optional: persisted to cert_dir/auth_secret.txt if omitted
shared_secret = "replace-me-for-production"

[internal_grpc.bootstrap]
# optional: persisted to cert_dir/bootstrap_token.txt if omitted
token = "replace-me-for-bootstrap"
```

Operational notes:

- `internal_grpc.enabled` must be `true` on the main control plane if you want
  to create or control remote-target agents, or if operators/scripts will use
  `agenthub actor ...` against the authority node.
- `tls` is the default recommended starting point. `mtls` is available when you
  want client-certificate verification as well.
- Keep `internal_grpc.auth.shared_secret` explicitly in `config.toml` when
  local actor CLI commands use the loopback control plane. The server can
  persist a generated secret under `cert_dir/auth_secret.txt`, but the CLI
  client only reads the config file when minting its token.
- The node registry stores routing metadata only (`grpc_target`,
  `tls_server_name`, `default_worktree_root`). It does not store node bootstrap
  secrets.
- Remote-target agent creation fails fast when internal gRPC peer config is not
  available, so this should be treated as a deployment precondition rather than
  a runtime toggle.

#### Recommended rollout order

1. Bring up the main AgentHub control plane with `internal_grpc.enabled = true`.
2. Bring up each remote AgentHub node with the same internal gRPC auth/security
   policy.
3. Verify the remote node exposes an `https://` internal gRPC endpoint that is
   reachable from the main control plane.
4. Log into the main AgentHub UI as root and register the remote node from the
   `Agents` page.
5. Set `Default worktree root` if the remote node should derive blank
   `create_worktree` workdirs automatically.
6. Create a remote-target agent and confirm the agent card shows `node:<id>`.
7. Start the agent and verify output/events are visible from the main control
   plane.

## Recommended Network Shape

- Reverse proxy terminates TLS and forwards to AgentHub
- AgentHub listens on internal/private address where possible
- Users access a single stable URL (for login, UI, and API)

## Basic Startup Commands

If you build a release binary:

```bash
./agenthub -c /path/to/config.toml
```

If you run from source:

```bash
cargo run -- -c /path/to/config.toml
```

## Post-Deploy Smoke Checklist

1. Open AgentHub UI and verify login works.
2. Create one agent with a safe test path.
3. Start a short task and confirm status reaches a terminal state.
4. Refresh browser and verify session history still exists.
5. Confirm a path outside `safe_paths` is rejected.
6. If distributed node mode is enabled, register one remote node and verify a
   remote-target agent can start and stream output back through the main
   control plane.

## Related Pages

- [Production Checklist](./production-checklist.md)
- [Troubleshooting](../operations/troubleshooting.md)
- [Configuration Basics](../getting-started/configuration-basics.md)
