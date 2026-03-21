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

## Notes

- `src/internal/client.rs` still keeps the faster in-process transport regression coverage for tight feedback loops
- the standalone blackbox workflow is now the authoritative p2p integration gate for cross-node mailbox relay
