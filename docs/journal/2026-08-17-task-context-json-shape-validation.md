# Summary

A code-only review of the Team subsystem found that `update_team_task`'s gRPC handler only validated
that its `context_json` field (a full `Replace` patch to a task's context) was *valid* JSON, unlike its
`context_merge_json` sibling, which additionally checks it's an object. Storing a non-object value this
way -- an array, `null`, a bare string -- planted a landmine: the next time that task's linked run
transitioned to `in_progress`, `run_task_status_sync.rs`'s `compute_next_task_execution_context` would
panic on `.as_object_mut().expect(...)`, crashing an otherwise-unrelated `create_run` (or any other
run-status-changing) call.

# Scope

- `src/internal/service/rpc.rs`: `update_team_task` now rejects a non-object `context_json` with
  `invalid_argument("context_json must be a JSON object")`, mirroring the existing
  `context_merge_json` check.
- `src/team/manager/run_task_status_sync.rs`: `compute_next_task_execution_context` no longer assumes
  its input is an object. If the stored context isn't one, it starts from `{}` instead of panicking.

# Key Decisions

- **Fixed both the ingestion gap and the panic itself**, not just one. The RPC-layer validation closes
  the only known way to write a non-object context, but `TeamManager::update_task_with_context` (the
  underlying manager API `resolve_task_context_patch` sits behind) has no such guard of its own, and
  `TeamTaskContextPatch::Replace` is a `pub` variant any future caller could construct without going
  through the RPC layer at all. Making the run-sync path tolerate a non-object context defends against
  both that and any row that predates this fix, without needing to push the same validation into every
  current and future caller of the manager API.
- Self-healing to `{}` (rather than, say, leaving a non-object context in place and skipping the
  attempt-number bump) matches this file's existing philosophy for corrupted/unexpected stored data
  (`sync_linked_task_status_tx` already does `unwrap_or_else(|_| json!({}))` on a JSON *parse* failure a
  few lines up) -- this closes the matching gap for a value that parses fine but has the wrong shape.

# Validation

- New `internal_grpc_team_task_update_rejects_non_object_context_json` (`context_tasks.rs`) asserts an
  array, `null`, and a string are all rejected with `InvalidArgument`.
- New `create_run_does_not_panic_when_linked_task_context_is_not_an_object` (`task_cases.rs`)
  reproduces the exact original bug end-to-end: bypasses the RPC-layer validation via the manager API
  directly (simulating a row that predates this fix), then calls `create_run` linking that task and
  asserts it completes without panicking, with the task correctly transitioning to `in_progress` and the
  self-healed context still tracking `execution.attempt_number`. Confirmed this test panics with the
  original message (`team task context should always be a JSON object`) when the
  `run_task_status_sync.rs` fix is reverted.
- `cargo test --lib team::manager::tests::task_cases` (14 passed),
  `cargo test --lib internal::service::tests::context_tasks` (13 passed).
- `cargo test --lib team::` (211 passed), `cargo test --lib internal::` (68 passed) -- no regressions.
- `cargo test --lib` -- 765 passed; 2 pre-existing `state::tests::*` failures (unrelated
  `lance-namespace-impls` panic) confirmed present on `main` before this change.
- `cargo clippy --lib --tests -p agenthub` and `cargo fmt -p agenthub -- --check` clean.

# Follow-Ups

- The other findings from the same 2026-08-17 Team-subsystem review round remain open, tracked in
  `docs/todo.md`'s Agent Team Correctness item.
