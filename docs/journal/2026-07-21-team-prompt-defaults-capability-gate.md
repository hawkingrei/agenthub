# Team Prompt Defaults Capability Gate

## Summary

The Team prompt defaults route now uses the `runtime:inspect` user capability instead of plain
authenticated-user authorization. Viewers can read the default coordinator and worker prompts;
device principals are denied before the route returns runtime prompt configuration.

## Background

The access-control rollout is migrating normal operator routes from coarse authentication-only gates
to product capability gates. Runtime defaults already use `runtime:inspect`, but the Team prompt
defaults endpoint still allowed any authenticated user to inspect runtime prompt text.

## Scope

- Converted `GET /api/teams/prompt_defaults` to `runtime:inspect`.
- Added router coverage proving a `device` user is denied with `runtime:inspect required`.
- Added router coverage proving a `viewer` user can still read the Team prompt defaults.

## Key Decisions

- Treat Team prompt defaults as runtime inspection because the route exposes runtime prompt
  configuration without mutating Team state.
- Keep Team creation, mutation, run, task, and upload routes for later capability-cluster slices.

## Validation

```bash
cargo test -p agenthub api::teams::tests::teams_router_http_contract -- --nocapture
cargo test -p agenthub api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles -- --nocapture
cargo fmt -p agenthub -- --check
git diff --check
```

## Follow-Ups

- Continue migrating the remaining Team management, task, run, and upload routes from
  authentication-only authorization to explicit capability gates.
