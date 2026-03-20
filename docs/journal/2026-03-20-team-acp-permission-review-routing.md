# Team ACP Permission Review Routing

## Summary

- Added Team-aware ACP permission review routing metadata to permission records.
- Worker-originated ACP permission requests now route to leader first through Team mailbox review.
- If agent review is unavailable or times out, the request falls back to `Channel` (`all`) for
  human review.
- Human review remains valid in parallel and does not block the original Team workflow.
- Double-review races now return `already_resolved` instead of silently reporting false success.
- Leader delegation now changes the active reviewer; non-delegated Team members may not approve a
  permission request.

## Backend Changes

- `crates/agenthub-acp/src/lib.rs`
  - added permission routing metadata and dispatcher trait
  - extended permission records with Team review-routing fields
  - changed `respond(...)` to compare-and-set on `pending`
  - `respond(...)` now reports `Applied` vs `AlreadyResolved` so callers can surface races correctly
  - tightened optional-column decoding so nullable review fields stay `None`
- `crates/agenthub-db/src/lib.rs`
  - extended `acp_permission_requests` schema with Team review-routing columns
- `src/agent/manager.rs`
  - threaded an optional permission-review dispatcher into ACP session spawn
- `src/team/permission_review.rs`
  - added Team dispatcher: `worker -> leader`
  - dispatches `permission_review_request` through Team mailbox
  - supports fallback notification into shared-thread `all`
- `src/actor_mcp.rs`
  - added `acp_permission_review_respond`
  - reviewers must be Team members and may not review their own request
  - only leader or the current delegated reviewer may approve a pending request
  - leader forwarding `permission_review_request` through `actor_send` reassigns the active reviewer
- `src/api/agents.rs`
  - human permission-response route now validates that the permission belongs to the addressed agent
  - repeated human approval now returns `already_resolved` instead of a false `ok`

## Prompt And Skill Updates

- `crates/agenthub-team-prompts/src/lib.rs`
- `skills/team/AGENTS.md`
- `skills/team/team-leader-orchestrator.SKILL.md`
- `skills/team/team-worker-executor.SKILL.md`
- `skills/team/team-actor-mailbox.SKILL.md`
- `web/src/pages/team/member_helpers.ts`

Added contract lines for:

- worker-originated Team ACP permission review routes to leader first
- reviewers use `acp_permission_review_respond`
- timed-out/unavailable agent review falls back to `Channel` (`all`) for human review

## Validation

- `cargo test acp_permission_review_respond_updates_pending_team_request -- --nocapture`
- `cargo test acp_permission_review_respond_reports_already_resolved_for_second_reviewer -- --nocapture`
- `cargo test leader_delegation_updates_active_reviewer_and_blocks_other_members -- --nocapture`
- `cargo test respond_permission_route_rejects_permission_from_other_agent -- --nocapture`
- `cargo test respond_permission_route_reports_already_resolved_after_first_response -- --nocapture`
- `cargo test dispatches_worker_permission_to_leader_and_can_fallback_to_human_review -- --nocapture`

## Follow-up

- Validate on deployed Team runtime that a live worker ACP permission request reaches leader
  mailbox first and appears in `all` only after agent-review timeout/failure.
- Consider tightening reviewer authorization from "any non-requester Team member" to explicit
  leader assignment once leader-to-worker permission delegation is formalized.
