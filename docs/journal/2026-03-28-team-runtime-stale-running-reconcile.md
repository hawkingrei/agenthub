# Summary

Aligned Team runtime reads with the single-agent stale-running reconciliation path so a member that
already exited no longer keeps the Team workbench stuck on `running`.

## Why

The single-agent surfaces already reconciled database rows that still said `running` even though no
runtime handle existed in memory. Team runtime reads did not do that reconciliation first; they
looked directly at `agents.status` and `agent_sessions.ended_at IS NULL`.

That let a crashed or already-exited Team member keep `/api/teams/:id/runtime` and Team context
reads in a stale `running` state until some other lifecycle action touched the row.

## Changes

- Added a Team API runtime-read preflight that runs `AgentManager::reconcile_runtime_absence(...)`
  for each configured member before returning `/api/teams/:id/runtime`.
- Added the same reconcile step to internal Team context reads so `describe_team_context` and
  derived internal controls observe the same member status truth as the Team page.
- Added focused regressions for both public Team runtime reads and internal gRPC context reads.

## Validation

- `cargo test -p agenthub teams_api_runtime_reconciles_stale_running_member_sessions -- --nocapture`
- `cargo test -p agenthub internal_grpc_describe_team_context_reconciles_stale_running_member_sessions -- --nocapture`

## Follow-up

- Verify the deployed Team workbench against a real stale-session scenario on
  `agenthub.hawkingrei.com` and record the browser-visible result here.
