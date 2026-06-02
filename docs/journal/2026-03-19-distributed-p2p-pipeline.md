# Distributed P2P Pipeline

## Summary

- promoted distributed actor mailbox p2p coverage from an in-process module test to a standalone blackbox integration pipeline
- added `tests/distributed_p2p_pipeline.rs` to boot two real `agenthub` processes with separate homes, databases, and web/internal listen addresses
- kept the existing in-process gRPC relay test, but renamed it so it is no longer confused with the main p2p pipeline
- added a dedicated GitHub Actions workflow `Distributed P2P Pipeline`

## What The Blackbox Test Covers

- two independent AgentHub nodes with separate `HOME` roots and SQLite state
- shared cluster-level internal gRPC auth/TLS material
- background remote relay worker delivery instead of direct `relay_remote_messages_once(...)` calls
- seeded bidirectional mailbox traffic (`A -> B` and `B -> A`)
- remote delivery state on the source node
- destination inbox visibility over internal gRPC
- destination ack over internal gRPC
- final delivered state and route stripping in destination local mailbox rows
- node stdout/stderr capture for CI artifact upload on failure

## CI

- new workflow: `Distributed P2P Pipeline`
- command:

```bash
cargo test --locked --test distributed_p2p_pipeline -- --nocapture
```

## P0 Closure Evidence (2026-06-02)

The phase 0/1 rollout gates are green on both the final PR pass and the post-merge
`main` push for commit `1d973f4a4b4bdee861863a973f77123c79ec9bea`.

PR #706 evidence:

- Distributed P2P Pipeline: `https://github.com/hawkingrei/agenthub/actions/runs/26807605199/job/79029012765`
- Bazel Test (Root): `https://github.com/hawkingrei/agenthub/actions/runs/26807605152/job/79029012880`
- Bazel Test (Crates): `https://github.com/hawkingrei/agenthub/actions/runs/26807605152/job/79029012741`
- Rust (Cargo): `https://github.com/hawkingrei/agenthub/actions/runs/26807605183/job/79029012943`
- Rust (Proto Check): `https://github.com/hawkingrei/agenthub/actions/runs/26807605183/job/79029012952`

Post-merge `main` evidence:

- Distributed P2P Pipeline: `https://github.com/hawkingrei/agenthub/actions/runs/26808585470/job/79032335858`
- Rust (gRPC Integration): `https://github.com/hawkingrei/agenthub/actions/runs/26808585469/job/79032334692`
- Rust (Proto Check): `https://github.com/hawkingrei/agenthub/actions/runs/26808585469/job/79032334770`
- Bazel Test (Root): `https://github.com/hawkingrei/agenthub/actions/runs/26808585629/job/79032335506`
- Bazel Test (Crates): `https://github.com/hawkingrei/agenthub/actions/runs/26808585629/job/79032335566`

This closes the active P0 rollout item for remote agent control, mailbox relay
plus ack, node-local data isolation, the blackbox distributed p2p pipeline, and
wire-compatibility coverage. Remaining distributed-node work stays tracked as
P1 follow-ups for startup boundaries, token-first join, deployment docs, and
transport posture.

## Notes

- `src/internal/client.rs` still keeps the faster in-process transport regression coverage for tight feedback loops
- the standalone blackbox workflow is now the authoritative p2p integration gate for cross-node mailbox relay
