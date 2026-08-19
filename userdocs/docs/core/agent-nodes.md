---
sidebar_position: 2
---

# Agent Nodes and Remote Execution

Agent Nodes let the main AgentHub control plane start and supervise an agent on
another machine. Users keep one web UI while repositories, credentials, and
compute remain on the selected execution host.

## Topology

```text
Browser
   |
   | HTTPS + SSE
   v
Main AgentHub control plane
   |
   | authenticated TLS/mTLS gRPC
   v
Remote AgentHub node -> local workdir -> ACP subprocess
```

The main process serves login, UI, API, Teams, and SSE. A process configured
with `server.role = "node"` serves internal gRPC only and does not expose the
public UI or `/health` HTTP endpoint.

## Registry Fields

| Field | Purpose |
|-------|---------|
| `id` | Stable unique node identity; must match the remote `server.node_id`. |
| `name` | Operator-facing label. |
| `grpc_target` | Reachable private `https://` internal gRPC endpoint. |
| `tls_server_name` | Optional expected certificate server name. |
| `default_worktree_root` | Optional base for generated remote worktrees. |

The registry stores routing metadata only. Bootstrap tokens, JWT shared
secrets, and TLS private keys remain deployment secrets.

## Main Control Plane

Enable internal gRPC when the main process operates remote agents or serves the
actor CLI:

```toml
[internal_grpc]
enabled = true
listen = "127.0.0.1:50051"

[internal_grpc.security]
mode = "tls"
cert_dir = "/etc/agenthub/internal-grpc"

[internal_grpc.auth]
shared_secret = "<deployment-secret>"
issuer = "agenthub"
audience = "agenthub-internal"

[internal_grpc.bootstrap]
token = "<bootstrap-token>"
```

Bind to a private reachable address rather than loopback when remote nodes must
connect. Use firewall policy to restrict the listener to expected peers.

## Remote Node

Install the same AgentHub release and configure a unique identity:

```toml
[server]
role = "node"
node_id = "node-east"

[internal_grpc]
enabled = true
listen = "0.0.0.0:50051"

[internal_grpc.security]
mode = "tls"
cert_dir = "/etc/agenthub/internal-grpc"

[internal_grpc.auth]
shared_secret = "<same-deployment-secret>"
issuer = "agenthub"
audience = "agenthub-internal"

[internal_grpc.bootstrap]
token = "<bootstrap-token-from-main>"
```

AgentHub does not restrict which workdir an agent can use on the node; scope
the remote service account's filesystem permissions to the repositories and
worktree roots it should touch.

The bootstrap token authenticates onboarding. Runtime calls still depend on
the configured TLS and JWT auth contract. Keep clocks synchronized and restart
processes after changing security material.

## Register and Validate a Node

1. On the main UI, sign in as root and open **Nodes** or
   **Agents → Join node with token**.
2. Copy the displayed bootstrap/security details through a secure channel.
3. Configure TLS material, auth values, the unique node ID, paths, and provider
   credentials on the remote host.
4. Start the remote node and confirm its internal gRPC port is listening.
5. Register its `https://` target, TLS server name, and optional default
   worktree root in the main UI.
6. Create an agent, select the remote node, and use a repository path that
   exists on that host.
7. Start a bounded task and verify live output, history replay, stop, and a
   second start.

The UI reserves node management for root operators. The backend protects node
routes with the `nodes:manage` capability, while bootstrap detail remains
root-only.

## Worktree Behavior

- Paths are resolved on the remote node; a main-host path has no meaning unless
  the remote filesystem exposes the same location.
- `use_existing` requires an existing allowed directory on the node.
- `create_worktree` requires a repository path/ref on the node and either an
  explicit workdir or the registered default worktree root.
- `reuse_worktree` requires a pre-existing Git worktree on the node.
- The remote service account needs Git, filesystem permissions, ACP provider
  binaries, and provider credentials.

## Actor and Mailbox Traffic

The same internal gRPC control plane carries remote lifecycle and actor mailbox
operations. Mailbox relay is at-least-once and uses message identity plus a
timestamp window for deduplication/staleness checks. Preserve message IDs and
timestamps when diagnosing delivery, and keep node clocks synchronized.

`agenthub actor ...` also requires internal gRPC on the authority process. For
local CLI use, keep `shared_secret` explicitly available through the effective
configuration; the CLI cannot assume a separately generated secret file.

## TLS Modes

- `tls` verifies the server certificate and still uses the internal JWT auth
  contract.
- `mtls` additionally requires client-certificate verification; it does not
  remove the need to keep the runtime auth configuration aligned.
- `disabled` is only for isolated local testing.

Before registration, inspect the certificate without bypassing verification:

```bash
openssl s_client \
  -connect node-east.internal:50051 \
  -servername node-east.internal \
  -CAfile /path/to/ca-cert.pem </dev/null
```

Do not use `grpcurl -insecure` as production evidence.

## Troubleshooting

| Symptom | Check |
|---------|-------|
| Connection failure | Node process/role, private routing, firewall, target host/port. |
| Certificate failure | CA trust, SAN, expiry, and registered `tls_server_name`. |
| Unauthorized | Bootstrap token during onboarding; shared secret, issuer, audience, and clock during runtime. |
| Agent starts locally instead | Selected execution node and persisted `target_node_id`. |
| Worktree failure | Repository/ref, default root, and permissions on the remote host. |
| Stale node signal | Process health, network path, and last-seen timestamp before editing registry state. |
| Node cannot be deleted | Stop or migrate agents that still reference it. |

Keep browser/API diagnostics on the main control plane and service/gRPC/process
diagnostics on the remote host. Do not expose the node listener publicly or
weaken TLS to compensate for a routing error.

See [Remote Node Transport](../deployment/remote-node-transport.md) for the
deployment security contract and [Workdir and Worktree Strategy](./workdir-worktree-strategy.md)
for workspace ownership.
