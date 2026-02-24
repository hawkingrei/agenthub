# Team Module File Split

## Summary

Split oversized Team backend source files into focused submodules so each file
stays under 1000 lines and remains reviewable.

## Background

`src/team/manager.rs` and `src/api/teams.rs` accumulated orchestration logic,
mailbox logic, and large test blocks in single files, which made navigation and
review difficult.

## Scope

- `src/team/manager.rs`
- `src/team/manager/codec.rs`
- `src/team/manager/mailbox.rs`
- `src/team/manager/tests.rs`
- `src/api/teams.rs`
- `src/api/teams/tests.rs`
- `src/api/teams/tests_core.rs`
- `src/api/teams/tests_router.rs`
- `docs/todo.md`

## Key Decisions

- Keep behavior unchanged; this is structure-only refactor.
- Split Team manager into:
  - core run/team lifecycle (`src/team/manager.rs`)
  - actor mailbox + relay adapter/store (`src/team/manager/mailbox.rs`)
  - row/status codec helpers (`src/team/manager/codec.rs`)
  - tests (`src/team/manager/tests.rs`)
- Split Team API tests into dedicated files:
  - shared fixtures/helpers (`src/api/teams/tests.rs`)
  - API behavior tests (`src/api/teams/tests_core.rs`)
  - router contract tests (`src/api/teams/tests_router.rs`)

## Validation

```bash
cargo test remote_actor_messages_relay -- --nocapture
cargo test team_run_messages_api_supports_actor_mailbox_flow -- --nocapture
cargo test teams_router_http_contract -- --nocapture
```

## Follow-ups

- Confirm full-suite CI (`cargo test --all`) after module split.
