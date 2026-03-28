# Internal P2P Core Crate

## Summary

- extracted the dependency-free internal p2p transport core into `crates/agenthub-internal-p2p`
- moved credential issuance helpers, node transport metadata, broadcast planning, and transport traits into the new crate
- kept `src/internal/p2p/mod.rs` as a thin adapter for root-crate-only concerns: `TeamActorMessageRecord` metadata extraction and `AgentNodeRecord` endpoint conversion

## Why This Slice First

- `src/internal/client` and `src/internal/service` still depend on root-crate `agent`, `team`, and `AppState` surfaces, so extracting them directly would introduce a dependency cycle or force a much riskier facade rewrite in one step
- `p2p` already had a clean protocol core; separating it first creates a real crate boundary now and reduces the amount of root-only code left under `src/internal`

## Validation

- `cargo fmt --all`
- `cargo check --locked`
- `cargo test --locked build_message_metadata_prefers_route_fields -- --nocapture`
- `cargo test --locked node_scoped_broadcast_planner_groups_members_by_target_node -- --nocapture`

## Follow-up

- next phase should target the control-service dependency graph, likely by introducing narrower internal-control facades for the `agent` / `team` operations that `TeamInternalControlService` actually uses
