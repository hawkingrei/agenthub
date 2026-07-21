# Auth Route Capability Guard

## Summary

The API authorization test suite now fails if production API route modules call `require_user` as a
route-level gate. Routes must use `require_capability` or a documented `require_root` gate.

## Background

The access-control rollout migrated normal operator routes from authentication-only checks to
capability gates. After the agent upload and OpenAPI JSON slices, production API route modules no
longer call `require_user(&headers, &state)` directly; only the canonical authz helpers and test
helpers should authenticate without a route capability.

## Scope

- Added a static API source guard for route-level `require_user` calls.
- Kept `require_root` allowed because security-critical or break-glass routes remain intentionally
  root-only during the migration.
- Kept `require_user` available inside `api::authz` because `require_root` and
  `require_capability` both build on session authentication.

## Key Decisions

- Guard production API source modules rather than test modules so test helpers can still mint and
  validate sessions directly.
- Fail on the concrete `require_user(&headers, &state)` and `require_user(headers, state)` route
  gate shapes instead of banning the symbol globally.
- Leave remaining migration work focused on non-route helpers and intentionally root-only security
  settings rather than normal operator routes.

## Validation

```bash
cargo test -p agenthub api::authz::tests::api_routes_do_not_use_authentication_only_user_gate -- --nocapture
cargo test -p agenthub api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles -- --nocapture
cargo fmt -p agenthub -- --check
git diff --check
```

## Follow-Ups

- Continue auditing non-route helper surfaces and intentionally root-only security settings before
  closing the access-control migration TODO.
