# A2A Agent Team Phase 1

## Summary

Introduce the first data-plane for Agent Team support based on A2A concepts:
team definitions, team runs, run steps, and globally ordered run events.

## Background

AgentHub currently manages single-agent runs. To support agent teams with A2A,
we need durable team/run state and replayable event streams before wiring in
remote execution.

## Decision

- Add persistent team tables:
  - `team_definitions`
  - `team_runs`
  - `team_steps`
  - `team_run_events`
- Use `team_run_events.id` (SQLite autoincrement) as the authoritative ordering
  for team replay and pagination.
- Add Team API endpoints for create/list/get team, create/get/cancel run, and
  run event listing.
- Keep execution orchestration out of phase 1; this phase only provides stable
  storage and API contracts.

## Scope

- `src/db.rs`
- `src/team/mod.rs`
- `src/team/manager.rs`
- `src/state.rs`
- `src/api/mod.rs`
- `src/api/teams.rs`

## Validation

- [x] Create a team and list it via `/api/teams`.
- [x] Create a run and verify it starts in `submitted`.
- [x] Cancel a run and verify status becomes `canceled`.
- [x] Query `/api/teams/runs/:run_id/events` and verify event order is stable by `event_id`.
- [x] Run `cargo test` and verify new `team::manager` tests pass.
