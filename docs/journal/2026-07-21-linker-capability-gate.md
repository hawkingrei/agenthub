# Linker Capability Gate

## Summary

The admin linker routes now use the `linkers:manage` user capability instead of the root-only
compatibility gate. Root and admin users can manage linker configuration and linked-resource reads;
operators and lower roles are denied by the capability matrix.

## Background

The access-control rollout is migrating normal operator routes by capability cluster. Node
management already moved to `nodes:manage`; linker configuration and resource reads are the next
cluster in the stable access-control contract.

## Scope

- Converted `/admin/linkers` and `/admin/linkers/slock*` routes to `linkers:manage`.
- Kept instance security and break-glass admin routes on the root-only compatibility gate.
- Updated the Slock linker route test to prove `operator` is denied and `admin` can configure and
  list linkers without exposing stored secrets.

## Key Decisions

- Treat linker configuration and linked Slock resource reads as one `linkers:manage` route cluster.
- Preserve existing resource-contract behavior for unimplemented Slock channel reads; authorization
  now reaches that contract only after the caller has `linkers:manage`.
- Keep safe paths, passkey settings, device revocation, join challenges, audit export, and archive
  migration outside this slice because they remain instance-security or root-owned maintenance
  actions.

## Validation

```bash
cargo test -p agenthub api::admin::tests::slock_linker_routes_require_linkers_manage_and_do_not_expose_secrets -- --nocapture
cargo test -p agenthub api::admin::tests::slock_link_attempt_requires_config_and_persists_state -- --nocapture
cargo test -p agenthub api::admin::tests::slock_channel_routes_report_missing_resource_contract -- --nocapture
cargo test -p agenthub api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles -- --nocapture
cargo fmt -p agenthub -- --check
git diff --check
```

## Follow-Ups

- Continue the route-cluster migration with the remaining normal agent and Team routes that still
  use authentication-only checks.
- Add audit records that include the capability name once the audit schema grows a stable field for
  capability evidence.
