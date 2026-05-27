# Team Permission Review Routing Verification

## Summary

- Closed the Team ACP permission review routing backlog item after re-checking the backend routing,
  internal review guardrails, human fallback behavior, and browser-only human review card UX.
- The current implementation now covers the intended end-to-end contract:
  - worker requests prefer an idle peer worker when available, otherwise fall back to another peer
    worker or the coordinator
  - coordinator requests route to a subordinate worker
  - the requester cannot review its own permission request
  - timed-out or failed agent review falls back to a human-visible shared-thread card with inline
    actions and a local alert tone

## Background

- Earlier Team permission review slices landed the routing metadata, mailbox dispatch, timeout
  coordination, and browser card UX in separate PRs and journals.
- `docs/todo.md` still carried one umbrella validation item covering the combined runtime contract.
- This checkpoint re-verified that the current `main` branch already contains the required behavior
  and focused regression tests, so the remaining work was documentation and backlog cleanup rather
  than another product change.

## Scope

- Verified backend routing and reviewer authorization surfaces under:
  - `src/team/permission_review/dispatcher.rs`
  - `src/team/permission_review/selection.rs`
  - `src/internal/service/rpc.rs`
- Verified focused backend regression tests under:
  - `src/team/permission_review/tests.rs`
  - `src/internal/service/tests/permission_review.rs`
- Verified browser-facing human review card coverage under:
  - `web/src/pages/team_panels.test.tsx`
- Removed the completed backlog item from `docs/todo.md`.

## Key Decisions

- Keep the routing policy actor-first and runtime-derived instead of introducing another explicit
  reviewer-assignment layer:
  - worker request -> idle peer worker if possible, otherwise next valid non-requester candidate
  - coordinator request -> subordinate worker only
- Preserve the strict no-self-review guard in internal gRPC even when legacy records need reviewer
  resolution fallback.
- Treat the human review card as the canonical timeout/failure fallback surface. It must remain
  actionable in the shared thread and should not depend on the stale agent-review mailbox message
  staying pending.

## Validation

```bash
cargo test -p agenthub permission_review::tests:: -- --nocapture
cargo test -p agenthub internal_grpc_permission_review_respond_ -- --nocapture
cd web && npm exec vitest run src/pages/team_panels.test.tsx --testNamePattern="permission review|plays a tone only when a new human permission review card arrives"
```

Additional code-backed verification points:

- `src/team/permission_review/tests.rs`
  - `dispatches_worker_permission_to_idle_peer_worker_before_busy_peer`
  - `dispatches_coordinator_permission_to_subordinate_worker`
  - `dispatch_failure_falls_back_to_human_permission_card`
- `src/internal/service/tests/permission_review.rs`
  - `internal_grpc_permission_review_respond_reports_timeout_before_reviewer_check`
  - `internal_grpc_permission_review_respond_keeps_pending_reviewer_guard`
  - `internal_grpc_permission_review_respond_rejects_requester_self_review`
- `web/src/pages/team_panels.test.tsx`
  - removes approved permission review cards after response
  - hides timed out cards before polling catches up
  - plays a tone only when a new human permission review card arrives

## Follow-Ups

- The remaining Team runtime backlog should move to the next open items in `docs/todo.md`,
  especially mailbox phase 3, prompt-tail slimming, and long-horizon context/memory continuity.
