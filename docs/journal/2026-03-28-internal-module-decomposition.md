# Internal Module Decomposition

## Summary

- Split `src/internal/p2p.rs` into `src/internal/p2p/{credentials,metadata,transport,broadcast,tests}.rs` with the existing `crate::internal::p2p::*` surface preserved by `mod.rs`.
- Split `src/internal/client.rs` into `src/internal/client/{mod,control,mailbox,tests}.rs` so connection/bootstrap logic, control RPC wrappers, mailbox/P2P transport glue, and tests no longer live in one file.
- Split `src/internal/service.rs` into `src/internal/service/{mod,rpc,helpers}/` plus grouped tests under `src/internal/service/tests/`.

## Why This Stop Point

- `p2p` is structurally ready for a future crate extraction, but `service` still depends directly on root-crate `AppState`, `agent`, and `team` modules.
- Moving `TeamInternalControlService` into a standalone crate immediately would create a dependency cycle unless the control boundary is first inverted around narrower domain traits or extracted manager crates.
- This change therefore focuses on file/module boundaries first while keeping public call sites unchanged.

## Validation

- `cargo fmt --all`
- `cargo check --locked`
- `cargo test helper_parsers_cover_known_and_default_values -- --nocapture`
- `cargo test internal_grpc_team_context_and_task_controls_are_wire_compatible -- --nocapture`
- `cargo test node_scoped_broadcast_planner_groups_members_by_target_node -- --nocapture`

## Follow-up

- If we still want a dedicated internal gRPC crate, extract a control-domain facade away from `AppState` first, then move `client` / `service` in a second phase.
