# Team Run Access And Memory Flush Hardening

## Background

PR review for Team context/memory baseline identified five follow-up items to improve security and maintainability:

1. Fix IDOR risk in `POST /api/teams/runs/:run_id/context/flush`.
2. Enforce owner checks consistently across run-scoped Team APIs.
3. Simplify API trigger normalization logic for memory flush.
4. Deduplicate repeated text truncation helper logic.
5. Split the large `TeamManager::flush_run_context` flow into clearer helper units.

## Scope

This change set updates Team run API authorization paths, memory-flush API normalization, shared utility reuse, and manager-side memory-flush internals. It does not introduce new user-facing API fields.

## Key Decisions

- Run ownership guard is centralized in API helpers:
  - Added `load_run_for_user` and `load_run_and_team_for_user`.
  - Replaced run existence-only checks with owner-aware checks for run-scoped routes.
  - Updated team-scoped run creation/listing to reuse `load_team_for_user` instead of direct `get_team`.
- Manual memory flush trigger normalization is now explicit `match`-based parsing in API layer for better readability and lower branch ambiguity.
- Introduced shared crate `crates/agenthub-text` with `truncate_chars`:
  - Reused by `src/team/manager.rs`, `src/team/orchestrator.rs`, and `crates/agenthub-acp/src/actor_runtime_skill.rs`.
  - Removed duplicated local truncation implementations from those files.
- Refactored `TeamManager::flush_run_context` by extracting query/result helpers:
  - Request normalization helper.
  - Team/run checkpoint/event loaders.
  - Failure/noop finalization helpers.
  - Checkpoint upsert helper.
  - Pointer payload builder.
  - Main path now focuses on orchestration and persistence sequence.

## Validation

Executed local checks:

- `cargo fmt --all`
- `cargo check`
- `cargo test team_runs_api_enforces_team_owner_access`
- `cargo test flush_run_context_`
- `cargo test team_runs_api_supports_manual_context_flush`
- `cargo test team_runs_api_rejects_invalid_context_flush_trigger`
- `cargo test -p agenthub-acp actor_runtime_skill`
- `cargo test -p agenthub-text`

## Follow-ups

- Keep the broader Team ACL matrix work deferred as documented in `docs/todo.md` (low-priority backlog item).
- Verify Bazel `//...` in CI after new local crate wiring (`agenthub-text`) to ensure Cargo/Bazel dependency alignment remains stable.
