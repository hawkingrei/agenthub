# Internal gRPC Service Narrow Dependencies

## Summary

- replaced the `TeamInternalControlService` runtime dependency on full `AppState` with an explicit `TeamInternalControlDeps` bundle
- limited that bundle to the four domains the service actually uses today: `db`, `agents`, `teams`, and `acp_permissions`
- updated the server bootstrap path plus internal service/client tests to construct the narrow bundle directly

## Why This Slice

- the previous split into `src/internal/service/{mod,rpc,helpers,tests}` only changed file boundaries; the service still imported root-crate `AppState`, which blocked further movement into a dedicated crate boundary
- this refactor removes the broad root-state dependency without introducing a new trait abstraction layer yet
- keeping the dependency bundle concrete preserves a small diff and lets the remaining internal gRPC control surface move incrementally

## Validation

- `cargo test -p agenthub 'internal::service::tests::' -- --nocapture`
- `cargo test -p agenthub 'internal::client::tests::' -- --nocapture`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`

## Follow-up

- if we continue the crate split, the next extraction target should be a smaller control-domain API around the `AgentManager` and `TeamManager` methods still consumed by `src/internal/service/rpc.rs`
- `src/internal/client` and `src/internal/service` now share a narrower dependency surface, but they still reference root-domain types directly and are not ready to move as-is into a standalone crate
