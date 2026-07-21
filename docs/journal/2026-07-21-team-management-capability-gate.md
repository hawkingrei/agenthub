# Team Management Capability Gate

## Summary

Team definition and channel mutation routes now use the `teams:manage` user capability instead of
plain authenticated-user authorization. Operators can manage Team definitions; viewers are denied
before mutation handlers change Team state.

## Background

The access-control rollout is migrating normal operator routes from coarse authentication-only gates
to product capability gates by route cluster. Team prompt defaults already moved to
`runtime:inspect`; this slice covers Team management mutations without changing Team runtime
operation, read-only inspection, tasks, runs, or upload routes.

## Scope

- Converted Team create, spec update, and delete routes to `teams:manage`.
- Converted Team channel create and delete routes to `teams:manage`.
- Added router coverage proving a `viewer` user is denied with `teams:manage required`.
- Added router coverage proving an `operator` user can create and delete a Team.

## Key Decisions

- Treat Team definition and channel mutations as Team management because they alter persistent Team
  structure.
- Leave Team start, stop, and force-new-session routes for a separate `runtime:operate` slice.
- Leave Team list/get/runtime/shared-thread inspection, tasks, runs, and uploads for separate route
  classification slices.

## Validation

```bash
cargo test -p agenthub api::teams::tests::teams_router_http_contract -- --nocapture
cargo test -p agenthub api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles -- --nocapture
cargo fmt -p agenthub -- --check
git diff --check
```

## Follow-Ups

- Continue migrating Team runtime controls to `runtime:operate`.
- Continue classifying Team read-only inspection, task, run, and upload routes by capability.
