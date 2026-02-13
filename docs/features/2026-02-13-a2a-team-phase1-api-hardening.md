# A2A Team Phase 1 API Hardening

## Summary

Harden Team Phase 1 API behavior with deterministic HTTP status mapping and
add both handler-level and router-level executable tests for lifecycle and
event ordering behavior.

## Background

Phase 1 introduced Team storage and API contracts, but error semantics were
still generic `500` in not-found and duplicate-name paths. The validation
checklist also needed runnable coverage to keep A2A progress trackable.

## Scope

- `src/api/error.rs`
- `src/api/teams.rs`
- `docs/features/2026-02-12-a2a-agent-team-phase1.md`
- `docs/todo.md`

## Key Decisions

- Map `sqlx::Error::RowNotFound` to `404` for team/run lookups.
- Map duplicate `team_definitions.name` to `409` conflict.
- Validate non-empty team names at API boundary (`400` bad request).
- Validate `spec` schema at API boundary:
  - `spec` must be an object,
  - `spec.entrypoint` must be a non-empty string,
  - `spec.members` must be a non-empty array of objects,
  - each `spec.members[].member_id` must be a non-empty unique string.
- Add cross-field constraints for scheduler compatibility:
  - when `spec.steps` is omitted, `spec.entrypoint` must reference a member id,
  - when `spec.steps` is present, `spec.entrypoint` must reference a step key,
  - each `spec.steps[].member_id` must reference an existing member id,
  - each `spec.steps[].depends_on` key must reference an existing step key,
  - `spec.steps` must be acyclic.
- Add spec compatibility versioning:
  - default missing `spec.spec_version` to `1` on create,
  - reject unsupported versions at create time,
  - re-validate persisted team spec before creating runs to prevent legacy
    incompatible specs from entering execution.
- Pre-check team/run existence in run creation and event listing to keep
  not-found behavior deterministic.
- Add handler-level API tests inside `src/api/teams.rs` that verify:
  - auth guard behavior,
  - team create/list/get,
  - duplicate-name conflict,
  - invalid `spec` payload rejection,
  - run create/cancel lifecycle,
  - event order and `before_id` pagination,
  - missing team/run returns `404`.
- Add router-level HTTP contract tests (`Router::oneshot`) to verify:
  - route matching and auth header handling,
  - JSON payload/response shape at wire level,
  - status code mapping remains stable through the full Axum stack.

## Validation

```bash
cargo test teams_api -- --nocapture
cargo test team_runs_api_supports_lifecycle_and_event_pagination -- --nocapture
cargo test teams_router_http_contract -- --nocapture
cargo test team::manager -- --nocapture
```

## Follow-ups

- Implement team step lifecycle persistence and event emission (`submitted` ->
  `working` -> terminal states) for scheduler bootstrap.
