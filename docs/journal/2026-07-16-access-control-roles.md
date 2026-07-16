# Access Control Roles

## Summary

Added a canonical access-control and user-role spec. The spec adapts the role-to-capability matrix
pattern into AgentHub's existing Rust/API/runtime boundaries without copying another project's
implementation shape.

## Background

The current repository already has:

- coarse browser/API auth with `require_user` and `require_root`;
- `root` and `device` user roles in the auth store;
- internal runtime tokens with roles, scopes, and explicit action permissions;
- Team member roles (`coordinator`, `worker`) used for execution semantics.

The missing contract is a human-facing user role and capability model that can replace broad
root-only gates over time while keeping Team/runtime roles separate.

## Scope

- Added `docs/features/access-control-and-roles.md`.
- Linked the spec from the feature index, architecture map, and journal summary.
- Added role/capability domain helpers and matrix tests.
- Added API capability auth helpers and a focused bypass guard for direct human-role route checks.
- Converted remote-node agent creation from a direct root check to the `nodes:manage` capability.
- Added a TODO item for the remaining route-cluster migration.

## Key Decisions

- Use capabilities as the stable authorization contract; routes should ask for capabilities rather
  than inspect role strings directly.
- Keep identity layers separate: human API users, device users, Team runtime members, and internal
  runtime token principals.
- Introduce a v1 user-role target of `root`, `admin`, `operator`, `viewer`, and `device`.
- Preserve root-only behavior for security-critical settings while migrating normal operation to
  capability gates.
- Require matrix tests, route behavior tests, and a bypass guard for authorization changes.

## Validation

Validation:

```bash
git diff --check
cargo test -p agenthub-auth-domain
cargo test api::authz
cargo test api::agents::tests::create_agent_route_rejects_remote_target_without_node_capability
```

## Follow-Ups

- Implement role/capability domain helpers and matrix tests.
- Add a bypass guard preventing new direct user-role checks outside the canonical authz module.
- Convert the first route cluster from root-only to capability auth after the matrix is in place.
