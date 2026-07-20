# Settings Runtime Defaults Capability Gate

## Summary

The runtime defaults settings route now uses the `runtime:inspect` user capability instead of plain
authenticated-user authorization. Viewers can read the configured default worktree root; device
principals are denied before the route returns runtime configuration.

## Background

The access-control rollout is migrating normal operator routes from coarse authentication-only gates
to product capability gates. Agent runtime inspection routes already use `runtime:inspect`, but the
settings route that exposes runtime defaults still accepted any authenticated user, including device
principals.

## Scope

- Converted `GET /api/settings/defaults` to `runtime:inspect`.
- Added route coverage proving a `device` user is denied with `runtime:inspect required`.
- Added route coverage proving a `viewer` user can still read runtime defaults.

## Key Decisions

- Treat runtime default settings as runtime inspection because the route exposes runtime placement
  configuration without mutating instance state.
- Keep root-only instance settings unchanged; this slice only covers read-only runtime defaults.

## Validation

```bash
cargo test -p agenthub api::settings::tests::runtime_defaults_requires_runtime_inspect_capability -- --nocapture
cargo test -p agenthub api::settings::tests::runtime_defaults_allows_viewer_runtime_inspect -- --nocapture
cargo test -p agenthub api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles -- --nocapture
cargo fmt -p agenthub -- --check
git diff --check
```

## Follow-Ups

- Continue migrating the remaining Team, task, upload, and other normal operator routes from
  authentication-only authorization to explicit capability gates.
