# Diagnostics Capability Gate

## Summary

The debug-only `/api/diagnostics/agent_trace` route now uses the `diagnostics:read`
capability instead of the root-only compatibility gate.

## Background

The access-control contract allows `root` and `admin` users to read diagnostics while
denying `operator`, `viewer`, and `device` users. The route still used the root-only
helper, which kept this read-only diagnostics surface stricter than the capability
matrix and blocked the route-cluster migration.

## Scope

- Switched the diagnostics route authz call to `require_capability(..., DiagnosticsRead)`.
- Added router coverage proving a `viewer` is denied and an `admin` passes authz.
- Kept the route debug-only and left trace target/resource validation unchanged.

## Key Decisions

- Treat diagnostics trace collection as `diagnostics:read`, not `instance:configure`.
- Use the existing capability matrix and authz helper rather than adding a route-local
  role check.
- Keep the test independent of live runtime trace fixtures by using a missing target
  after authz succeeds.

## Validation

```bash
cargo test -p agenthub api::diagnostics::tests::agent_trace_requires_diagnostics_read_capability -- --nocapture
cargo test -p agenthub api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles -- --nocapture
cargo fmt -p agenthub -- --check
```

## Follow-Ups

- Continue migrating the next route cluster from root-only gates to capability gates.
- Keep security-critical instance settings on root-only authorization unless the stable
  access-control contract changes.
