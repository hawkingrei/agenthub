# Cargo Clippy Zero Warning and CI Gate

## Summary

Drive the workspace to a zero-warning Clippy baseline and add a dedicated CI
pipeline that enforces `-D warnings`.

## Background

The repository already had Rust/Bazel/E2E workflows, but no standalone
`cargo clippy` gate. As a result, lint regressions could slip in even when
build/test pipelines were green.

At the same time, several warning categories (`collapsible_if`,
`redundant_closure`, `if_same_then_else`, `too_many_arguments`,
`large_enum_variant`, and enum naming) accumulated across core modules.

## Scope

- `.github/workflows/clippy.yml`
- `crates/agenthub-acp/src/lib.rs`
- `agenthub-codex-acp/src/thread.rs`
- `src/team/manager/mailbox.rs`
- `src/internal/auth.rs`
- `src/internal/service.rs`
- `src/agent/manager.rs`
- `src/api/agents.rs`
- `docs/todo.md`

## Key Decisions

1. Add a dedicated `Clippy` GitHub Actions workflow:
   - install system deps
   - pin Rust toolchain to `1.93.1`
   - run `cargo clippy --workspace --all-targets -- -D warnings`
2. Keep Rust/Bazel/E2E workflows split by responsibility; clippy checks are
   enforced in their own pipeline.
3. Replace high-arity function signatures with input structs where meaningful:
   - `spawn_acp_session(SpawnAcpSessionRequest)`
   - `send_actor_message(SendActorMessageInput)`
4. Remove enum-size warning by boxing the large submission variant:
   - `SubmissionState::Prompt(Box<PromptState>)`
5. Remove internal action naming warning by dropping repeated `Team` prefixes
   in `InternalAction` variants while preserving permission string values.
6. Keep permission response behavior explicit:
   - `option_id` drives `Selected`
   - otherwise validate `outcome` and map to `Cancelled`

## Validation

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected:

- Clippy exits successfully with zero warnings.
- New `Clippy` workflow can be used as an independent PR quality gate.

## Follow-ups

- Wire branch protection to require the `Clippy` workflow for merge.
