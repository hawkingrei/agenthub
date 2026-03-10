# Team Runtime Endpoint And Fixture Fix

## Summary

Added an explicit Team runtime read model and router endpoint so Team lifecycle can be observed independently from run state:

- `GET /api/teams/:id/runtime`
- shared lifecycle helpers under `src/team/runtime.rs`
- `TeamManager::describe_team_runtime(...)`

The endpoint reports Team-level runtime state (`stopped` / `degraded` / `running`) from member live sessions instead of inferring it from run state.

## Why

The Team model was already being moved toward:

- member-owned persistent runtime sessions
- run-owned task/step execution state

The backend still lacked a direct Team runtime projection, so `/teams` and router tests had to infer status indirectly.

While adding the endpoint, router tests exposed a real fixture mismatch:

- seeded Team member agents used `/usr/bin/env`
- `create_team` auto-started the member runtimes successfully
- but the subprocesses exited immediately
- `GET /runtime` therefore degraded to `degraded`

That fixture contradicted the actual product contract for Team member runtimes, which are expected to stay alive.

## Implementation

### Backend runtime read model

- Added `TeamRuntimeRecord`
- Added `TeamRuntimeMemberRecord`
- Added `TeamManager::describe_team_runtime(...)`
- Added `GET /api/teams/:id/runtime`

The read model joins:

- Team spec members
- agent rows
- currently running agent sessions
- member cards

and derives the Team status:

- `stopped`: no online member sessions
- `running`: all members have live sessions
- `degraded`: partial coverage

### Shared lifecycle helpers

Moved Team lifecycle helpers out of `src/api/teams.rs` into:

- `src/team/runtime.rs`

so create/start/stop logic is shared and easier to evolve with the Team runtime model.

### Test fixture correction

Updated the default seeded Team member agents used by API/router tests to run the real local actor runtime entrypoint:

- command: current `agenthub` binary
- args: `["actor-mcp"]`

instead of `/usr/bin/env`.

This makes the fixture consistent with persistent Team member runtimes and allows router tests to assert `running` after `create_team`.

## Validation

Targeted checks used for this change:

```bash
cargo test actor_mcp -- --nocapture
cargo test describe_team_runtime_returns_member_runtime_status -- --nocapture
cargo test teams_router_http_contract -- --nocapture
cargo test teams_api_create_team_auto_starts_member_runtime -- --nocapture
```

## Follow-up

- Keep the deployed `/teams` verification item in `docs/todo.md` as the UI-side confirmation for runtime controls and runtime badge copy.
- The next architectural step remains the deeper Team runtime lifecycle work:
  - persistent Team-owned member sessions
  - explicit Team start/stop UX
  - orchestrator as work dispatcher only
