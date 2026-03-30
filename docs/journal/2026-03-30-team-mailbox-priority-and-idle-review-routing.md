# Team Mailbox Priority And Idle Review Routing

## Summary

- Made Team mailbox priority classes explicit as `general`, `urgent`, and
  `permission_review`.
- Kept existing immediate-hint behavior for urgent mailbox traffic.
- Added idle-first reviewer selection for Team ACP permission review so busy
  peer workers are not interrupted when an idle reviewer is available.

## Scope

- `src/team/mailbox_hint.rs`
- `src/team/permission_review.rs`
- `docs/features/actor-foundation.md`
- `docs/features/agents-teams.md`
- `docs/todo.md`

## Key Decisions

1. Preserve the existing urgent/general behavior instead of redesigning the
   whole mailbox transport.
   - direct `agent -> agent` sends remain `urgent`;
   - leader channel messages with explicit mentions remain `urgent`;
   - other mailbox traffic remains `general` and keeps the delayed unread
     summary path.
2. Treat Team ACP permission review as its own priority class.
   - reviewer selection now checks candidate reviewers in priority order and
     prefers one whose ACP session has been idle for the shared mailbox idle
     window;
   - if no idle reviewer exists, the dispatcher falls back to the previous
     non-self reviewer order (`worker -> peer worker -> leader`,
     `leader -> subordinate worker`).
3. Reuse the existing ACP-idle signal instead of inventing a new one.
   - the dispatcher reuses the same "no recent non-user ACP output" heuristic
     already used by delayed unread mailbox summaries.
4. Keep permission-review nudges best-effort.
   - once a reviewer is chosen, AgentHub still sends one immediate mailbox hint;
   - a failed hint must not fail the review dispatch itself.

## Validation

```bash
cargo test -p agenthub team::mailbox_hint::tests::actor_mailbox_priority_classes_are_stable -- --nocapture
cargo test -p agenthub team::permission_review::tests::collect_permission_review_candidates_keeps_leader_as_fallback_after_workers -- --nocapture
cargo test -p agenthub team::permission_review::tests::dispatches_worker_permission_to_idle_peer_worker_before_busy_peer -- --nocapture
cargo fmt --all
git diff --check
```

## Follow-up

- Verify on a deployed Team session that:
  - normal mailbox traffic still waits for the unread-summary idle window;
  - direct mailbox sends and leader mentions still nudge immediately;
  - a permission review prefers an idle peer worker before interrupting a busy
    one when both are eligible reviewers.
