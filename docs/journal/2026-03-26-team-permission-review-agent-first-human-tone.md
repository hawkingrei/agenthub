# Team Permission Review Agent-First Routing And Human Alert Tone

## Summary

- Worker-originated Team ACP permission reviews now prefer another non-requester agent reviewer
  before falling back to leader review.
- Team human-review fallback cards now trigger a short browser-local alert tone when a new pending
  card first appears in the conversation UI.

## Why

- Requiring worker-originated reviews to go through leader first is stricter than necessary and
  slows down routine approvals when another worker can review safely.
- Human fallback is intentionally the last resort; once it happens, the operator should notice
  it immediately without having to keep the Team conversation in constant visual focus.

## What Changed

- Team reviewer selection now uses one shared resolver for both mailbox dispatch and
  `respond_permission_review` validation.
- For worker-originated requests, the shared resolver prefers a peer worker reviewer when one is
  available; if not, it falls back to leader review.
- Legacy permission requests without a stored `review_target_actor_id` now reuse the same reviewer
  resolution logic instead of assuming `worker -> leader only`.
- `TeamTaskPanel` now tracks newly arrived pending `permission_review_card` messages and plays a
  short local alert tone once per permission id when the card first appears.
- Active Team verification backlog text now includes the local alert tone expectation.

## Validation

- `cargo test dispatches_worker_permission_to_peer_worker_before_human_review internal_grpc_permission_review_respond_accepts_legacy_team_peer_worker_fallback -- --nocapture`
- `cd web && npx vitest run src/pages/team_panels.test.tsx -t "TeamTaskPanel plays a tone only when a new human permission review card arrives"`
