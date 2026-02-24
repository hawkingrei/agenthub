# Team Domain Crate Migration (Phase 3)

## Background

`src/team/mod.rs` mixed Team domain data model definitions and runtime orchestration
exports. To progress from module split to crate-level boundaries, domain data
models should live in an isolated crate with explicit Cargo/Bazel edges.

## Scope

- Introduce a new crate: `crates/agenthub-team-domain`.
- Move Team domain models/constants/error type from `src/team/mod.rs` and
  `src/team/manager.rs` into the new crate.
- Keep runtime behavior unchanged by re-exporting domain types through
  `src/team/mod.rs`.
- Wire both Cargo workspace and Bazel dependency edges.

## Key Decisions

1. New crate `agenthub-team-domain` contains:
   - Team status constants (`TEAM_RUN_STATUS_VALUES`, `TEAM_STEP_STATUS_VALUES`)
   - Team records/config/status enums
   - `TeamRunResumeError`
2. Root app crate depends on the new crate and re-exports domain items from
   `src/team/mod.rs` to avoid broad call-site churn in this phase.
3. `src/team/manager.rs` now re-exports/uses `TeamRunResumeError` from the domain
   crate, preserving existing API mapping behavior.
4. Bazel boundary alignment:
   - add `//crates/agenthub-team-domain:agenthub_team_domain`
   - add root `agenthub_lib` dependency on this new target

## Validation

Executed locally:

```bash
cargo test -q -p agenthub-team-domain
cargo test -q teams_api_create_list_get_and_reject_duplicate_name
cargo test -q team_runs_api_supports_resume_and_restart_strategy
cargo test -q teams_router_http_contract
```

All passed.

Bazel note:

- Attempted:
  `bazel build //crates/agenthub-team-domain:agenthub_team_domain`
- Local environment failed before analysis due `rules_rust` repository resolution
  under local Bazel cache setup (`No MODULE.bazel ... @@rules_rust+//rust`).
- Follow-up verification is tracked in TODO for CI/clean environment.

## Follow-up

- Continue phase-3 migration by moving additional cohesive Team domain logic into
  crates with the same pattern.
- Verify new crate Bazel target on CI or clean local Bazel environment.
