# Node Mode Startup Boundary

## Summary

- introduced an explicit server runtime role:
  - `server.role = "main"` keeps the current public control-plane behavior
  - `server.role = "node"` starts only the node runtime surface
- added `server.node_id` and require it for `node` mode so internal gRPC peer identity no longer defaults to `main`
- stopped node mode from booting the public HTTP/UI server
- stopped node mode from creating main-only push notification state on startup

## Why

The previous startup path treated every `agenthub` process as the same kind of server:

- public HTTP/UI/API booted unconditionally
- internal gRPC peer identity defaulted to `main`
- node processes still initialized side effects meant for the main control plane

That broke the intended control-plane vs execution-node boundary in distributed deployments.

## Scope

- `crates/agenthub-config/src/lib.rs`
- `src/app.rs`
- `src/state.rs`
- `src/push.rs`
- `docs/features/agent-nodes.md`
- `userdocs/docs/getting-started/configuration-basics.md`
- `userdocs/docs/core/agent-nodes.md`
- `userdocs/docs/deployment/overview-and-topology.md`

## Validation

- `cargo test -p agenthub-config`
- `cargo test -p agenthub app::tests::validate_startup_config_ -- --nocapture`
- `cargo check -p agenthub`
