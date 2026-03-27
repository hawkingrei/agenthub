## Summary

- added explicit Team task assign / unassign support on the canonical Kanban task path
- kept assignment leader-owned and explicit; the runtime still does not guess task ownership
- exposed assignment through internal gRPC / actor CLI while keeping human UI/API task mutation disabled

## Why

The Team task-first model already carried `assigned_member_id`, but it was only a nullable storage
field:

- task creation always persisted `assigned_member_id = NULL`
- the update path only changed task status
- Kanban could not show ownership explicitly

That left a gap between the ownership contract and the actual product behavior.

## What Changed

### Backend

- expanded Team task update semantics from status-only to patch semantics:
  - optional `status`
  - optional `assigned_member_id`
  - explicit unassign support
- validate assigned members against `spec.members[].member_id`
- keep leader-only write ownership for canonical Team task updates

### CLI / internal control

- `agenthub actor team-task-update` now supports:
  - `--status <...>`
  - `--assigned-member-id <member_id>`
  - `--unassign`
- internal gRPC request mirrors the same patch semantics

### UI / public API

- Kanban task surfaces now show the current assignee as read-only context
- Kanban explains that status/ownership stay agent-managed through Team runtime controls
- public HTTP task mutation remains disabled for human clients

## Validation

- `cargo test -p agenthub task_assignment_updates_are_persisted -- --nocapture`
- `cargo test -p agenthub teams_api_rejects_human_task_status_and_owner_updates -- --nocapture`
- `cargo test -p agenthub parse_team_task_update_accepts_assignment_patch -- --nocapture`
- `cd web && npx vitest run src/pages/team_panels.test.tsx`
