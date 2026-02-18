# Team Member Role Required

## Background

Team specs previously accepted members without explicit `role`, and runtime logic inferred leader/worker from `leader_member_id` and fallback rules.  
This made Team semantics less explicit and increased ambiguity in spec review and long-term evolution.

## Scope

- Enforce explicit `spec.members[].role` in Team spec parsing.
- Restrict role values to `leader` or `worker`.
- Keep existing leader consistency checks (`leader_member_id` alignment and single-leader constraint).
- Update Team API/router tests to use explicit member roles in valid specs.

## Key Decisions

1. `role` is now required in every member entry.
   - Missing/empty/invalid values return `400`.
2. Allowed role values remain unchanged:
   - `leader`
   - `worker`
3. `leader_member_id` behavior remains unchanged:
   - at most one `members[].role = leader`;
   - when both are present, `leader_member_id` must match the leader role member.

## Implementation Notes

- `src/api/teams.rs`
  - `TeamMemberSpec.role` changed from optional to required string.
  - Replaced optional parser with strict `parse_required_member_role`.
  - Updated role usage in snapshot building and leader resolution paths.
- `src/api/teams/tests_core.rs`
  - Updated valid Team specs in API and run lifecycle tests to include explicit roles.
- `src/api/teams/tests_router.rs`
  - Updated router contract and orchestrator convergence test payloads to include explicit roles.

## Validation

- `cargo test teams_api_ -- --nocapture`
- `cargo test teams_router_ -- --nocapture`
- `cargo test team_run_ -- --nocapture`
- `cargo clippy --workspace --all-targets -- -D warnings`

Re-verified with current branch state:

- `cargo test teams_api_ -- --nocapture` (9 passed)
- `cargo test teams_router_ -- --nocapture` (2 passed)
- `cargo test team_run_ -- --nocapture` (8 passed)
