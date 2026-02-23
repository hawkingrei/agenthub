# Team Delete Member Session FK Hardening

## Background

`DELETE /api/teams/:id` could return `500` when Team member agents had runtime history rows linked to active sessions.

The delete flow removed `agent_sessions` for Team members before removing dependent rows in:

- `agent_events` (`session_id` FK to `agent_sessions.id`)
- `acp_permission_requests` (`session_id` FK to `agent_sessions.id`)

In SQLite with foreign keys enabled, this produced FK violations and the API surfaced a generic internal error.

## Scope

This change only hardens Team delete cleanup ordering and regression coverage.

No API contract changes, payload changes, or UI changes were introduced.

## Key Decisions

- Update Team delete cleanup order in `src/api/teams.rs`:
  - delete `acp_permission_requests` by `agent_id`
  - delete `agent_events` by `agent_id`
  - delete `agent_sessions` by `agent_id`
  - execute member cleanup and Team cascade deletion in a single database transaction boundary
- Extend `TeamManager::delete_team` in `src/team/manager.rs` to accept member IDs so Team delete keeps all DB-side cleanup (member runtime rows + Team domain rows) inside one transaction.
- Keep API handler focused on auth/ownership and agent stop attempts; DB mutation is delegated to TeamManager transactional path.
- Extend Team delete API regression test (`teams_api_delete_team_cascades_related_run_data`) to insert and verify cleanup of:
  - member `agent_events`
  - member `acp_permission_requests`
- Add router-level regression test (`teams_router_delete_team_cleans_member_session_dependents_without_500`) to verify `DELETE /api/teams/:id` no longer returns `500` under session-dependent rows.
- Keep test schema aligned with runtime schema by adding `acp_permission_requests` table in `src/api/teams/tests.rs`.

## Validation

Executed local checks:

- `cargo test teams_api_delete_team_cascades_related_run_data -- --nocapture`
- `cargo test delete_team -- --nocapture`
- `cargo test teams_router_delete_team_cleans_member_session_dependents_without_500 -- --nocapture`

Both passed after the cleanup-order fix.

## Follow-ups

- Verify the same path in CI (`push` + `pull_request`) and record workflow run IDs before closing the TODO item.
