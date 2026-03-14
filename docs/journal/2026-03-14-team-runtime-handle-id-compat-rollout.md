# Team runtime handle id compatibility rollout

## Summary

- promoted `runtime_handle_id` to the primary Team step field name in Rust domain and web types
- kept `remote_task_id` as a compatibility field on HTTP/proto responses and requests
- preserved the SQLite column name `remote_task_id` for now to avoid a schema migration in the same change

## Scope

- `crates/agenthub-team-domain`: `TeamStepRecord` now stores `runtime_handle_id`
- `src/team/manager*`: internal logic now reads/writes `runtime_handle_id` while still binding the legacy DB column
- `src/api/teams.rs`: start-step request now accepts `runtime_handle_id` and aliases legacy `remote_task_id`
- `web/src/api.ts` and Team panels now prefer `runtime_handle_id` while falling back to `remote_task_id`
- tests now verify the new field and compatibility alias behavior

## Compatibility policy

- database: keep `team_steps.remote_task_id` unchanged for now
- internal proto: keep `remote_task_id` unchanged for now
- HTTP start-step request: accept both `runtime_handle_id` and `remote_task_id`
- HTTP/team step responses: emit both `runtime_handle_id` and `remote_task_id`
- web clients: read `runtime_handle_id` first, then fall back to `remote_task_id`

## Follow-up

- if the compatibility period holds, the next phase can move internal proto and SQL schema naming toward `runtime_handle_id`
- that later phase should be a separate migration with explicit wire/storage compatibility planning
