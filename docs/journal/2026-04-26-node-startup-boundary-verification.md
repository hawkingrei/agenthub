# Node Startup Boundary Verification

## Summary

- tightened focused Rust coverage around node-vs-main startup boundaries
- confirmed the existing implementation already enforces the intended node-mode
  contract; the missing gap was regression coverage, not a new runtime fix

## Scope

- `src/app.rs`
- `src/state.rs`

## What Was Added

- `validate_startup_config_rejects_node_mode_without_node_id`
- `setup_database_creates_root_user_for_main_role`
- `setup_database_skips_root_user_for_node_role`
- `initialize_services_enables_push_for_main_role`
- `initialize_services_disables_push_for_node_role`

These tests lock the most important startup boundaries:

- node mode requires `internal_grpc.enabled = true`
- node mode requires an explicit non-`main` `server.node_id`
- node mode must not seed the root user
- node mode must not initialize push/VAPID state
- main mode keeps the existing root-user and push initialization behavior

## Validation

- `cargo test -p agenthub validate_startup_config_ -- --nocapture`
- `cargo test -p agenthub state::tests::setup_database_ -- --nocapture`
- `cargo test -p agenthub state::tests::initialize_services_ -- --nocapture`

## Follow-up

- This does not close the broader TODO by itself; push/PR CI evidence still
  needs to be recorded before the TODO can be removed.
