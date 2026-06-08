---
sidebar_position: 3
---

# Remote Node Transport

Use this page when deploying AgentHub with remote Agent Nodes.

AgentHub remote-node traffic currently uses authenticated `https://` gRPC on an
internal endpoint. Browser HTTP, API, and SSE traffic still belong to the main
control-plane web listener.

## Current Production Posture

The recommended deployment shape is:

- one `main` AgentHub process serving the web UI and API
- one or more `node` AgentHub processes serving internal gRPC only
- private-network `https://` gRPC targets for every registered node
- root-managed node registry rows for `grpc_target`, `tls_server_name`, and
  optional `default_worktree_root`
- shared internal gRPC auth/TLS configuration across the small cluster

Do not expose the node-only gRPC listener as a public browser endpoint. Node
mode is for execution and internal control traffic only.

## Required Transport Rules

Remote-node deployments should enforce these rules:

| Rule | Requirement |
|------|-------------|
| Endpoint scheme | Use `https://` for registered `grpc_target` values. |
| Registry authority | Register routes from the main control plane; do not rely on route JSON overrides. |
| TLS material | Store TLS files in internal gRPC config, not in node registry rows or relay payloads. |
| Bootstrap | Use `Agents -> Join node with token` to copy bootstrap details. |
| Runtime auth | Keep `issuer`, `audience`, and `shared_secret` aligned across peers today. |
| Dedupe | Treat remote delivery as at-least-once and make duplicate sends harmless. |
| Clock skew | Reject stale relay credentials or envelopes outside the allowed timestamp window. |

## Dedupe and Timestamp Window

Remote mailbox relay can retry. Receivers must therefore reject stale requests
and dedupe repeated delivery before processing the business payload.

Recommended policy:

- accept relay timestamps only inside a `+-120s` skew window
- use the transport `idempotency_key` as the canonical dedupe key
- for compatibility with old relay metadata, fall back to `(source_node_id,
  message_id)` only when no first-class `idempotency_key` is present
- retain accepted dedupe keys for at least `24h` or the configured retry horizon,
  whichever is longer
- return success for duplicates after skipping business processing
- log rejected deliveries with a reason such as `expired`, `duplicate`, or
  `signature_mismatch`

The goal is at-least-once transport with effectively-once mailbox effects.

## Dedicated Port vs Same-Port Multiplexing

The supported deployment mode today is a dedicated internal gRPC port, for
example:

```toml
[internal_grpc]
enabled = true
listen = "0.0.0.0:50051"
```

Same-port HTTP plus gRPC multiplexing is a future deployment simplification.
The intended design is:

- one TLS listener can accept browser HTTP/SSE and internal gRPC
- ALPN and `Content-Type: application/grpc` route gRPC requests to the internal
  service
- normal HTTP requests continue to route to the web/API stack
- internal gRPC authz stays the same as the dedicated-port mode
- node-only mode still avoids exposing the public web/API surface

Keep the dedicated-port mode until your reverse proxy and AgentHub build both
validate same-port behavior.

## Identity and mTLS Roadmap

The current small-cluster model uses a shared signing secret plus TLS or mTLS.
This keeps rollout simple, but it is not the final identity model.

The long-term path is:

1. keep stable `server.node_id` values and registry rows for every node
2. issue short-lived credentials from the main control plane with `node_id`,
   `audience`, `scope`, `issued_at`, and `expires_at`
3. bind production mTLS certificate identity to the same `node_id`
4. reject requests where token identity and certificate identity disagree
5. add rotation and revocation before treating per-node identity as the steady
   state trust root

Until that path is complete, treat the shared secret as sensitive cluster-wide
material and rotate it with the same care as an API signing key.

## Preflight Checklist

Before adding a remote node:

1. Confirm the main control plane has `internal_grpc.enabled = true`.
2. Confirm the remote process uses `server.role = "node"` and a non-`main`
   `server.node_id`.
3. Confirm the registered `grpc_target` is reachable from the main process.
4. Confirm the certificate validates with the configured `tls_server_name`.
5. Confirm auth `issuer`, `audience`, and `shared_secret` match on both peers.
6. Confirm clocks are synchronized with NTP or an equivalent time source.
7. Confirm the remote node's worktree root exists and is inside `safe_paths`.

## Smoke Test

After registration:

1. Create one remote-target agent from the `Agents` page.
2. Start it and verify the card shows `node:<id>`.
3. Send a short input and confirm output appears in the main UI.
4. Confirm the remote node keeps local runtime data while the main control plane
   keeps the catalog view.
5. Run a mailbox or Team flow that targets the remote agent and confirm the
   message is delivered and can be acknowledged.
6. Retry the same relay delivery in staging and confirm it does not create a
   duplicate destination mailbox row.

## Related Pages

- [Deployment Overview and Topology](./overview-and-topology.md)
- [Production Checklist](./production-checklist.md)
- [Agent Nodes and Remote Execution](../core/agent-nodes.md)
