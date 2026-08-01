# OpenAPI Capability Gate

## Summary

The OpenAPI JSON discovery route now uses the `runtime:inspect` user capability instead of plain
authenticated-user authorization. Device sessions are denied before receiving the API contract.

## Background

The access-control rollout is migrating normal operator routes from coarse authentication-only gates
to product capability gates by route cluster. The OpenAPI docs page remains public HTML, but
`GET /openapi.json` exposes the authenticated API contract and belongs with runtime inspection.

## Scope

- Converted `GET /openapi.json` to `runtime:inspect`.
- Added route coverage proving a device session is denied with `runtime:inspect required`.
- Preserved the existing unauthorized response and authorized OpenAPI JSON contract.

## Key Decisions

- Treat OpenAPI JSON as inspection rather than management because it returns API metadata without
  mutating state.
- Keep `/openapi/docs` public because it is static HTML that instructs the browser to use the user's
  stored token for the JSON request.
- Keep the route separate from agent upload authorization because upload routes are persistent
  agent-scoped writes.

## Validation

```bash
cargo test -p agenthub api::openapi::tests::openapi_json_requires_runtime_inspect_capability -- --nocapture
cargo test -p agenthub api::openapi::tests::openapi_json_contains_team_runs_list_path -- --nocapture
cargo test -p agenthub api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles -- --nocapture
cargo fmt -p agenthub -- --check
git diff --check
```

## Follow-Ups

- Continue auditing non-route helper surfaces and intentionally root-only security settings.
