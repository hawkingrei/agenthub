# API Teams Error Mapping Module Split

## Background

`src/api/teams.rs` carried both HTTP handlers and low-level error mapping helpers
(unique-constraint detection, row-not-found mapping, actor-service status mapping).
This mixed transport flow with mapping details and increased file size/maintenance cost.

## Scope

- Extract Teams API error mapping helpers from `src/api/teams.rs` into
  `src/api/teams/errors.rs`.
- Keep handler behavior and HTTP responses unchanged.
- Keep constants and constraint semantics unchanged.

## Key Decisions

1. Introduce `mod errors;` under Teams API module and re-export helper functions
   via `use self::errors::{...}` in `src/api/teams.rs`.
2. Move these helpers to `src/api/teams/errors.rs`:
   - `map_create_team_error`
   - `map_submit_step_error`
   - `map_actor_service_api_error`
   - `map_not_found_error`
   - `map_resume_run_error`
   - `map_team_internal_error`
   - unique/row-not-found detector helpers
3. Keep `SQLITE_CONSTRAINT_UNIQUE_CODE` source-of-truth in `teams.rs`, referenced
   from `errors.rs` via `super::SQLITE_CONSTRAINT_UNIQUE_CODE`.

## Validation

Executed locally:

```bash
cargo test -q teams_api_create_list_get_and_reject_duplicate_name
cargo test -q team_runs_api_supports_resume_and_restart_strategy
cargo test -q teams_router_http_contract
```

All passed.

## Follow-up

- Continue phase-2 split by extracting Team API payload structs and conversion
  helpers into dedicated submodules while preserving router behavior.
- Phase-3 will migrate cohesive API/service domains into `crates/*` with Bazel
  package alignment.
