# Distributed Node Architecture

## Summary

- added a canonical feature spec for phased distributed node architecture
- refined the roadmap from a coarse `phase 1/2/3` split into phases `0-6`
- documented the phase 1 shared-key baseline, phase 4 authenticated gossip membership, and phase 6 zero-trust identity upgrade
- fixed the architecture boundary that `gossip` is metadata-only while business mailbox/control payloads stay on authenticated `gRPC`
- documented node-scoped broadcast fanout for large team broadcasts

## Stable Decisions

- phase 0 freezes protocol fields and runtime abstractions before larger rollout steps
- phase 1 may use one cluster-wide shared signing key because the initial deployment size is small
- phase 2 introduces node-scoped broadcast fanout before gossip is added
- phase 3 introduces `MembershipView` before gossip-backed membership is added
- phase 1 still keeps `gRPC` as the `node <-> node` and `node <-> AgentHub` business transport
- `gossip` is not the mailbox transport; it is reserved for membership, health, load, capability, and small descriptors
- broadcast should aggregate by `target_node_id` before local fanout to team members
- protocol fields must stay forward-compatible with later per-node identity and scoped credential rollout

## Follow-Up Areas

- define the concrete `BroadcastIntent` / `NodeBroadcastEnvelope` / `NodeBroadcastAck` payload schema
- define the `MembershipView` abstraction and authenticated gossip payload schema
- define the phase 5 fanout scale controls and benchmarking targets
- define the phase 6 bootstrap, rotation, and revocation flow for per-node identity
