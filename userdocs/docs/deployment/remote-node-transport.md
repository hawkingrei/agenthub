---
sidebar_position: 3
---

# Remote Node Transport

Remote Agent Nodes execute agents on other machines while the main AgentHub
process remains the browser/API control plane.

## Supported Topology

- The `main` process serves HTTP, the web UI, SSE, and internal gRPC client
  operations.
- A `node` process serves internal gRPC only; it does not serve the public UI.
- Registered `grpc_target` values use `https://` on a dedicated private port.
- The registry stores routing metadata, not bootstrap tokens, JWT shared
  secrets, or TLS private keys.

Keep browser traffic on the main HTTP listener and node traffic on the internal
gRPC listener. Same-port HTTP/gRPC multiplexing is not the conservative
deployment contract documented here.

## Security Contract

| Control | Requirement |
|---------|-------------|
| TLS | Use `tls` or `mtls`; reserve `disabled` for isolated local tests. |
| Bootstrap | Copy the one intended bootstrap token from the main control plane and protect it as a secret. |
| Runtime auth | Keep issuer, audience, and shared-secret configuration aligned on every peer. |
| Identity | Give every node a unique `server.node_id` other than `main`. |
| Routing | Register the reachable `https://` target and expected TLS server name. |
| Time | Synchronize clocks because relay validation uses bounded timestamps. |

Mailbox relay is at-least-once. AgentHub uses idempotency and timestamp checks
to reject duplicate or stale delivery, but operators should still verify this
behavior in their network and proxy path.

## Node Configuration

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
shared_secret = "<deployment-secret>"
issuer = "agenthub"
audience = "agenthub-internal"

[internal_grpc.bootstrap]
token = "<bootstrap-token-from-main>"
```

The main control plane must also enable internal gRPC with matching security and
auth settings when it operates remote agents or serves `agenthub actor ...`.

## Rollout

1. Start the main control plane with internal gRPC enabled.
2. Copy the bootstrap token shown by **Agents → Join node with token**.
3. Configure and start the remote node with a unique node ID.
4. Verify its certificate and `https://` gRPC reachability from the main host.
5. As root, register the route and optional default worktree root in **Agents**.
6. Create an agent targeting that node.
7. Start it and confirm status, output, stop, and reconnect behavior from the
   main UI.

## Failure Triage

- `unauthorized`: compare bootstrap token and post-bootstrap JWT
  issuer/audience/shared secret.
- certificate verification failure: compare CA trust, certificate SAN, and the
  registered `tls_server_name`.
- connection failure: confirm the target is reachable from the main host and
  that the node process is actually in `server.role = "node"`.
- workdir failure: validate paths and permissions on the remote filesystem, not
  on the main host.
- duplicate/stale relay rejection: check clocks and preserve the message ID and
  timestamp evidence.

Do not weaken TLS or expose the node listener publicly as the first recovery
step.
