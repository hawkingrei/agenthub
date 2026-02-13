# Agent Manager File Split

## Summary

Split `src/agent/manager.rs` into submodules so each file stays under 1000
lines while preserving runtime behavior.

## Background

`agent/manager` mixed lifecycle orchestration, runtime process handling, helper
codec functions, and tests in one large file. This made review and long-term
maintenance harder, especially for local Agent runtime paths.

## Scope

- `src/agent/manager.rs`
- `src/agent/manager/runtime.rs`
- `src/agent/manager/codec.rs`
- `src/agent/manager/tests.rs`
- `docs/todo.md`

## Key Decisions

- Keep behavior unchanged; this is a structure-only refactor.
- Split by responsibility:
  - `manager.rs`: core API methods and startup paths.
  - `runtime.rs`: output streaming, exit watcher, process finalization, worktree
    preparation, and runtime cleanup methods.
  - `codec.rs`: status/stream/worktree/path/provider helper functions and
    command resolution helpers.
  - `tests.rs`: manager unit tests.
- Keep helper functions at `pub(super)` visibility to avoid exposing new public
  API surface.

## Validation

```bash
cargo test remote_actor_messages_relay -- --nocapture
cargo test team_run_messages_api_supports_actor_mailbox_flow -- --nocapture
cargo test teams_router_http_contract -- --nocapture
```

## Follow-ups

- Run full CI suite (`cargo test --all`) to validate cross-module integration
  paths not covered by targeted tests.
