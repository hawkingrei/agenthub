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
- One main SQLite database plus per-agent SQLite event databases
- Browser clients connected with HTTP + SSE

Optional scale-out deployments can add remote Agent Nodes for execution and
mailbox delivery while keeping AgentHub as the main control plane.

## Runtime Components

Prepare these components before rollout:

1. Matching `agenthub` and `agenthubd` release binaries (or a source build for
   development)
2. A valid `config.toml`
3. Writable runtime home (default under `~/.agenthub/`)

## Deployment Modes

### Single-machine mode

Use this for personal or small internal deployments. Install the release pair,
write `~/.agenthub/config.toml`, and start the server:

```bash
agenthubd
```

Release builds embed the web UI. Source builds and frontend development belong
to the contributor workflow, not the production deployment path.

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
- Keep the current production posture on a dedicated internal `https://` gRPC
  port rather than relying on same-port HTTP plus gRPC multiplexing.

#### Distributed node prerequisites

Every participating node runs the same `agenthubd` binary. The difference
is which node acts as the main control plane and which nodes are registered as
remote execution targets.

Recommended remote-node baseline:

```toml
[server]
role = "node" # main | node
node_id = "node-east"

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
shared_secret = "<shared-secret-from-your-secret-store>"

[internal_grpc.bootstrap]
# optional: persisted to cert_dir/bootstrap_token.txt if omitted
token = "<node-bootstrap-token>"
```

Operational notes:

- The main control-plane instance should keep the default `server.role = "main"`
  (or omit `server.role` entirely) so it continues to serve the public web/UI
  and API surface.
- `server.role = "node"` turns the process into a node-only runtime. In this
  mode AgentHub serves internal gRPC only and does not boot the public web/UI
  HTTP surface.
- `server.node_id` is required when `server.role = "node"` and must match the
  node id registered on the main control plane.
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
- Remote mailbox relay is at-least-once. Keep clocks synchronized and use the
  documented idempotency/timestamp policy so retries do not create duplicate
  destination mailbox rows.

#### Recommended rollout order

1. Bring up the main AgentHub control plane with `internal_grpc.enabled = true`.
2. Copy the bootstrap token/details from `Agents -> Join node with token`.
3. Configure each remote AgentHub node with that bootstrap token, the same
   internal gRPC auth/security policy, `server.role = "node"`, and a unique
   `server.node_id`.
4. Verify the remote node exposes an `https://` internal gRPC endpoint that is
   reachable from the main control plane.
5. Log into the main AgentHub UI as root and register the remote node from the
   `Agents` page.
6. Set `Default worktree root` if the remote node should derive blank
   `create_worktree` workdirs automatically.
7. Create a remote-target agent and confirm the agent card shows `node:<id>`.
8. Start the agent and verify output/events are visible from the main control
   plane.

## Recommended Network Shape

- Reverse proxy terminates TLS and forwards to AgentHub
- AgentHub listens on internal/private address where possible
- Users access a single stable URL (for login, UI, and API)

## Startup

AgentHub currently reads `~/.agenthub/config.toml`; there is no alternate
config-path flag. Start the installed release binary with:

```bash
agenthub
```

Use the Debian package's systemd unit for a managed Linux service. For source
development, follow the repository's contributor documentation.

## Post-Deploy Smoke Checklist

1. Open AgentHub UI and verify login works.
2. Create one agent with a safe test path.
3. Start a short task and confirm status reaches a terminal state.
4. Refresh browser and verify session history still exists.
5. If distributed node mode is enabled, register one remote node and verify a
   remote-target agent can start and stream output back through the main
   control plane.

## Related Pages

- [Production Checklist](./production-checklist.md)
- [Remote Node Transport](./remote-node-transport.md)
- [Troubleshooting](../operations/troubleshooting.md)
- [Configuration Basics](../getting-started/configuration-basics.md)
