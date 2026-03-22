# Team ACP Review Auto-Routing Tightening

## Summary

- Tightened Team ACP permission review so worker-originated permission requests stay leader-routed
  while leader-originated permission requests auto-route to a subordinate worker reviewer.
- Rejected manual mailbox forwarding of `permission_review_request` payloads through `actor_send`.
- Reduced Team skill/prompt leakage of the concrete approval tool name; Team contracts now describe
  ACP-side review flow instead of peer-delegated mailbox work.
- Kept the universal guardrail that no requester may review its own permission request.

## Implementation Notes

- `src/actor_mcp.rs`
  - `actor_send` now rejects system-managed `permission_review_request` payload forwarding.
  - approval is limited to the current automatically assigned reviewer.
  - requester actors may never approve their own request.
  - actor MCP `tools/list` only exposes the permission-review action to actors with an assigned
    pending Team permission review.
- `src/team/permission_review.rs`
  - permission-review dispatcher now auto-selects `worker -> leader` and `leader -> subordinate worker`
    reviewers without manual mailbox delegation.
- `skills/team/AGENTS.md`
- `skills/team/team-actor-mailbox.SKILL.md`
- `skills/team/team-leader-orchestrator.SKILL.md`
- `skills/team/team-worker-executor.SKILL.md`
- `crates/agenthub-team-prompts/src/lib.rs`
- `docs/features/agents-teams.md`
  - aligned Team contracts and prompts with automatic non-requester reviewer ownership.

## Validation

- `cargo test worker_permission_review_is_leader_only_and_cannot_be_forwarded -- --nocapture`
- `cargo test leader_permission_review_is_owned_by_assigned_worker_reviewer -- --nocapture`
- `cargo test acp_permission_review_respond_updates_pending_team_request -- --nocapture`
- `cargo test acp_permission_review_respond_reports_already_resolved_for_second_reviewer -- --nocapture`
- `cargo test jsonrpc_tools_list_exposes_permission_review_only_to_current_reviewer -- --nocapture`
- `cargo test jsonrpc_tools_list_and_call_drive_local_mailbox_flow -- --nocapture`

## Follow-up

- The current approval action still exists behind the actor MCP server for leader sessions.
- A future ACP-native review action can remove the remaining MCP-tool exposure entirely once ACP
  runtime supports a dedicated internal review control surface.
