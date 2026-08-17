# Summary

A code-only review of the Team subsystem found that the "current active reviewer" for a pending
permission review was resolved two different ways depending on *when* it was checked, and the two could
disagree: dispatch-time selection (`TeamPermissionReviewDispatcher::resolve_review_target`) is
idle-aware -- it picks the first candidate whose agent runtime looks idle, falling back to the first
candidate otherwise -- while the internal gRPC `respond_permission_review` handler, whenever
`review_target_actor_id` hadn't been persisted on the record yet, re-derived a reviewer via
`resolve_team_permission_review_target`, which is idle-*unaware* and deterministically returns the
first candidate. If those two selections differed (the first candidate wasn't idle, so dispatch picked
someone else) and a `respond_permission_review` call landed in the narrow window between dispatch
delivering the mailbox message and its own `record_review_dispatch` write committing, the actor who
actually received the review would be rejected while a different actor who was never notified could be
treated as authorized to approve or deny it.

# Scope

- `src/internal/service/rpc.rs`: `respond_permission_review` no longer re-derives a reviewer from the
  team spec when `review_target_actor_id` is absent. It now compares directly against the persisted
  column and fails closed (the same `permission_denied` this function already returns for a genuine
  wrong-actor mismatch) when no target has been recorded yet.
- `src/team/permission_review/selection.rs`: removed `resolve_team_permission_review_target`, the
  idle-unaware wrapper that was the only caller of this now-dead fallback path. Its underlying candidate
  logic (`collect_team_permission_review_candidates` -- worker-before-coordinator selection,
  role-trimming, self-review exclusion) is unchanged and still used by the dispatcher's idle-aware
  selection; only the thin "just take the first candidate" wrapper is gone.
- `src/team/mod.rs`, `src/internal/service/mod.rs`: dropped the now-dead re-exports.
- `src/internal/service/tests/permission_review.rs`: the three "legacy fallback" tests that specifically
  exercised the removed resolution path (`accepts_legacy_team_coordinator_fallback`,
  `accepts_legacy_team_peer_worker_fallback`, `surfaces_legacy_reviewer_resolution_errors`) are replaced
  by one test asserting the new fail-closed behavior
  (`respond_rejects_when_no_reviewer_target_persisted_yet`). Two other tests
  (`respond_updates_pending_request`, `respond_rejects_conflicting_outcome_fields`) that inserted a
  pending request via raw SQL without a `review_target_actor_id` -- relying on the old fallback to reach
  the behavior they actually meant to test -- now seed that column directly.
- `src/team/permission_review/tests.rs`: the four unit tests that called
  `resolve_team_permission_review_target` now call `collect_team_permission_review_candidates` directly
  and check its first candidate, preserving the same coverage of the underlying selection logic.

# Key Decisions

- **Fail closed instead of trying to make the fallback idle-aware too.** Making
  `respond_permission_review`'s fallback match the dispatcher's idle-aware selection would require
  wiring the same agent-runtime/mailbox-idle-check machinery (`TeamMailboxHintAgentNudger`) into
  `TeamInternalControlService`, which doesn't currently have it and isn't otherwise a natural dependency
  of that service. Since dispatch's `record_review_dispatch` write is the *only* path that ever persists
  `review_target_actor_id`, an absent value means dispatch either hasn't finished or never started --
  no agent should have received the review to respond to yet. Confirmed with the person requesting this
  fix that removing the fallback (rather than trying to duplicate its logic) is the correct scope, since
  every legitimate caller of this endpoint is an agent responding to a message it already received.
- **Deleted the dead function rather than keeping it `#[allow(dead_code)]`.** Its own defect (idle-blind
  selection) was the actual bug; keeping it around unused risked someone re-wiring it back in without
  realizing why it was removed. Its test coverage was preserved by retargeting to the shared
  candidate-collection function it used to wrap.

# Validation

- `cargo test --lib team::permission_review::` -- 12 passed.
- `cargo test --lib internal::service::tests::permission_review` -- 7 passed, including the new
  fail-closed regression test and the two raw-SQL tests updated to seed a persisted reviewer target.
- `cargo test --lib team::` -- 211 passed; `cargo test --lib internal::` -- 67 passed. No regressions.
- `cargo test --lib` -- 765 passed; the 2 pre-existing `state::tests::*` failures (unrelated
  `lance-namespace-impls` panic) were already present on `main` before this change.
- `cargo clippy --lib --tests -p agenthub` and `cargo fmt -p agenthub -- --check` clean.

# Follow-Ups

- The other findings from the same 2026-08-17 Team-subsystem review round remain open, tracked in
  `docs/todo.md`'s Agent Team Correctness item.
