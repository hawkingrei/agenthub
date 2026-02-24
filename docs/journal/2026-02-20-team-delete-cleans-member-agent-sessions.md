# Team Delete Cleans Member Agent Sessions

## Background

`DELETE /api/teams/:id` previously removed Team records (`team_definitions`, `team_runs`, `team_steps`, `team_run_events`, `team_actor_messages`) but left member agent sessions untouched.  
When Team members were mapped to real agent IDs, this could leave stale/running `agent_sessions` after Team deletion.

## Scope

- Update Team delete API flow to cleanup member runtime/session state before deleting Team data:
  - load Team spec and resolve `spec.members[].member_id`;
  - attempt `stop_agent(member_id)` for each member to stop active runtime handles;
  - delete `agent_sessions` rows for each member ID;
  - keep existing Team data cascade delete behavior unchanged.
- Add coverage in Team API delete test to assert member `agent_sessions` are removed.

## Key Decisions

- Session cleanup is implemented in API layer (`src/api/teams.rs`) where `AppState.agents` is available for runtime stop calls.
- Member ID parsing failure during delete does not block Team deletion; it falls back to empty member set and logs a warning.
- This change targets session lifecycle cleanup only; agent definition deletion policy remains unchanged.

## Validation

- `cargo test teams_api_delete_team_cascades_related_run_data -- --nocapture`
  - verifies Team cascade delete still works;
  - verifies `agent_sessions` for Team member IDs are deleted.
