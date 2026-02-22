# Team Run Resume And Restart Strategy

## Summary

Add run-level `resume` and `restart` APIs and wire Teams UI controls so users can
recover Team runs after interruptions without manual run recreation.

## Background

Team Workbench previously exposed run-level `cancel` and step-level `resume`, but
had no run-level recovery controls. After backend restarts or terminal run states,
operators had to manually create a new run and re-enter context/input.

## Scope

- `src/team/manager.rs`
- `src/team/manager/tests.rs`
- `src/api/teams.rs`
- `src/api/teams/tests.rs`
- `src/api/teams/tests_core.rs`
- `src/api/teams/tests_router.rs`
- `src/api/openapi.rs`
- `web/src/api.ts`
- `web/src/pages/team_page.tsx`
- `docs/todo.md`

## Key Decisions

1. Define run-level strategy in `TeamManager`:
   - `resume_run(run_id)`:
     - return existing run unchanged for active statuses (`submitted`,
       `working`, `input_required`)
     - fork a fresh submitted run for terminal recoverable statuses (`failed`,
       `canceled`) while preserving `team_id`, `context_id`, and `input`
     - reject `completed` with conflict semantics
   - `restart_run(run_id)`:
     - always fork a fresh submitted run with the same `team_id`,
       `context_id`, and `input`
2. Keep original run immutable during fork-based recovery to preserve historical
   auditability and event replay consistency.
3. Expose new HTTP endpoints:
   - `POST /api/teams/runs/{run_id}/resume`
   - `POST /api/teams/runs/{run_id}/restart`
4. Return `409 conflict` when resuming a completed run.
5. Add Teams Active Run controls (`Resume Run`, `Restart Run`) with conservative
   enablement:
   - `Resume Run`: enabled for `failed`/`canceled`
   - `Restart Run`: enabled for `completed`/`failed`/`canceled`

## Validation

```bash
cargo test -q resume_run_handles_active_terminal_and_completed_statuses
cargo test -q restart_run_creates_new_submission_with_same_context_and_input
cargo test -q team_runs_api_supports_resume_and_restart_strategy
cargo test -q teams_router_resume_restart_strategy_survives_state_reopen
cargo test -q teams_router_http_contract
cargo test -q openapi_json_contains_team_runs_list_path
npm --prefix web run test -- src/pages/team_page.runs.test.ts
npm --prefix web run test -- src/pages/team_panels.test.tsx
npm --prefix web run lint -- src/pages/team_page.tsx src/api.ts
```
