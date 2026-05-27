# Team Permission Review Routing Verification

## Summary

- Verified the remaining Team ACP permission review routing contract end to end.
- Added a focused internal gRPC regression test for the `no self-review` guard.
- Removed the corresponding open verification item from `docs/todo.md`.

## Background

- The routing behavior had already been implemented across earlier slices:
  - `worker -> idle peer worker when available, otherwise peer worker/coordinator fallback`
  - `coordinator -> subordinate worker`
  - timed-out or failed agent review falls back to a human-visible `permission_review_card`
  - the Team UI plays a short local alert tone when a new human-review card first appears
- The remaining gap was not a broad behavior hole; it was missing verification closure for the
  `requester must never review its own request` rule.

## Scope

- Keep the change bounded to verification and backlog cleanup.
- Do not change the routing algorithm or UI behavior.
- Add one targeted backend regression test to cover the missing `self-review` prohibition.

## Key Decisions

1. Treat this TODO as a verification item, not a new behavior rollout.
2. Close the gap with the narrowest meaningful test at the internal gRPC permission-review
   surface, because that is where Team reviewer authorization is enforced.
3. Remove the open backlog item once the missing guard is covered and the existing routing/tone
   tests still represent the rest of the contract.

## Validation

- `cargo test -p agenthub dispatches_worker_permission_to_idle_peer_worker_before_busy_peer -- --nocapture`
- `cargo test -p agenthub dispatches_coordinator_permission_to_subordinate_worker -- --nocapture`
- `cargo test -p agenthub dispatch_failure_falls_back_to_human_permission_card -- --nocapture`
- `cargo test -p agenthub internal_grpc_permission_review_respond_accepts_legacy_team_peer_worker_fallback -- --nocapture`
- `cargo test -p agenthub internal_grpc_permission_review_respond_accepts_legacy_team_coordinator_fallback -- --nocapture`
- `cargo test -p agenthub internal_grpc_permission_review_respond_rejects_requester_self_review -- --nocapture`
- `cd web && npx vitest run src/pages/team_panels.test.tsx -t "TeamTaskPanel plays a tone only when a new human permission review card arrives"`

## Follow-Ups

- Real-world long-session verification still remains under the broader ACP and Team browser matrix
  backlog, but the Team permission review routing contract itself is no longer an open TODO item.
