# Team Shared Thread Capability Gate

## Summary

The Team shared-thread ensure route now uses the `teams:manage` user capability instead of plain
authenticated-user authorization. Viewers are denied before the route can create or reuse the
canonical shared-thread task.

## Background

The access-control rollout is migrating normal operator routes from coarse authentication-only gates
to product capability gates by route cluster. Team read and preview routes are covered by
`runtime:inspect`, but `POST /{id}/shared_thread` is not a pure read: it may create the canonical
shared-thread task and conversation state for the Team.

## Scope

- Converted `POST /{id}/shared_thread` to `teams:manage`.
- Added router coverage proving a viewer is denied with `teams:manage required`.
- Added router coverage preserving the authorized ensure-and-read shared-thread contract.
- Preserved the existing Team ownership check and canonical shared-thread idempotency behavior after
  the capability gate.

## Key Decisions

- Treat shared-thread ensure as Team management because it can create Team task and conversation
  state.
- Keep `GET /{id}/shared_thread` under `runtime:inspect`; reading an existing shared thread remains
  inspection, while ensuring it exists is a mutation.
- Keep resource checks after capability checks so capability authorization does not replace Team
  ownership boundaries.

## Validation

```bash
cargo test -p agenthub api::teams::tests::teams_router_http_contract -- --nocapture
cargo test -p agenthub api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles -- --nocapture
cargo fmt -p agenthub -- --check
git diff --check
```

## Follow-Ups

- Continue auditing other API route clusters for authentication-only authorization.
