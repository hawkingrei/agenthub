# Remote Node Transport Posture

## Summary

Closed the remote-node transport posture TODO by moving the durable deployment
and protocol posture into canonical docs and user-facing deployment guidance.

## Background

Phase 0/1 distributed-node work already had runtime coverage for encrypted gRPC
remote agent control, mailbox relay, node-local data isolation, and the blackbox
distributed p2p pipeline. The remaining P1 tail was not another runtime change;
it was the missing operator-facing transport posture:

- relay dedupe and timestamp-window policy
- dedicated-port gRPC versus future same-port HTTP/gRPC multiplexing
- the production identity path from shared-secret phase 1 to per-node mTLS

## Scope

- `docs/features/distributed-node-architecture.md`
- `userdocs/docs/deployment/remote-node-transport.md`
- `userdocs/docs/deployment/overview-and-topology.md`
- `userdocs/docs/deployment/production-checklist.md`
- `userdocs/docs/core/agent-nodes.md`
- `userdocs/sidebars.js`
- `docs/todo.md`

## Key Decisions

1. The current canonical production posture is a dedicated internal `https://`
   gRPC endpoint for remote-node traffic.
2. Same-port HTTP plus gRPC multiplexing is a future-compatible design target,
   not the current conservative default.
3. Remote mailbox relay is at-least-once; receivers must enforce timestamp
   windows and idempotency before applying mailbox business effects.
4. The canonical dedupe key is transport `idempotency_key`; compatibility may
   fall back to `(source_node_id, message_id)` when legacy metadata lacks a
   first-class idempotency key.
5. The recommended timestamp skew window is `+-120s`, with accepted dedupe keys
   retained for at least `24h` or the configured retry horizon.
6. The long-term identity path is main-issued short-lived node credentials plus
   mTLS certificate identity bound to the same `node_id`.

## Validation

```bash
git diff --check
cargo fmt --check
npm --prefix userdocs run build
```

Existing runtime evidence remains the phase 1 gate:

```bash
cargo test --locked --test distributed_p2p_pipeline -- --nocapture
```

## Follow-Ups

- Implement and validate same-port HTTP plus gRPC multiplexing before changing
  the recommended deployment default.
- Add per-node credential issuance, rotation, revocation, and certificate
  identity binding before replacing the shared-secret trust root.
