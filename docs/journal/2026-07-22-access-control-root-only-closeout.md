# Access Control Root-Only Closeout

## Summary

The user-role capability migration has a reviewed root-only closeout guard.
Normal API route operations now use capability gates, while the remaining
`require_root` calls are locked to reviewed security-sensitive boundaries.

## Background

Earlier July slices migrated route clusters from broad root-only checks to
named capabilities. The remaining TODO tail was to audit non-route helpers and
intentionally root-only settings before closing the migration.

## Scope

- Keep API route authentication-only guard coverage in `src/api/authz.rs`.
- Keep direct human-role checks out of production API route modules.
- Add a reviewed `require_root` allowlist guard for security boundaries.
- Leave Team member roles, internal runtime roles, device identity handling,
  and auth-domain role parsing outside the human API route migration.

## Key Decisions

- `safe_paths`, passkey settings, device revocation/audit listing, node join
  bootstrap material, VAPID key inspection/rotation, and archive backfill
  migration remain root-only.
- Linkers, agent/node CRUD, runtime inspect/operate, Team management, Team
  mailbox, Team tasks, uploads, OpenAPI, diagnostics, push subscription, and
  runtime settings stay on named capability gates.
- Future `require_root` route calls must be added to the reviewed allowlist,
  so new root-only behavior is explicit instead of accidental.
- Provider, Team member, device, and internal runtime role checks are separate
  identity layers and are not human API authorization bypasses.

## Validation

```bash
cargo test --lib api::authz::tests::api_root_only_gates_stay_on_reviewed_security_boundaries
cargo test --lib api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles
cargo test --lib api::authz::tests::api_routes_do_not_use_authentication_only_user_gate
```

## Follow-Ups

- Add audit-record persistence for any new security-sensitive route mutation in
  the same PR that introduces it.
- Revisit root-only classification if non-root role-management UI becomes a
  product requirement.
