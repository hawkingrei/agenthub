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
- Extend Team delete API regression test (`teams_api_delete_team_cascades_related_run_data`) to insert and verify cleanup of:
  - member `agent_events`
  - member `acp_permission_requests`
- Keep test schema aligned with runtime schema by adding `acp_permission_requests` table in `src/api/teams/tests.rs`.

## Validation

Executed local checks:

- `cargo test teams_api_delete_team_cascades_related_run_data -- --nocapture`
- `cargo test delete_team -- --nocapture`

Both passed after the cleanup-order fix.

## Follow-ups

- Verify the same path in CI (`push` + `pull_request`) and record workflow run IDs before closing the TODO item.
