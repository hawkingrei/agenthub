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
- Converted agent-node management routes from root-only access to the `nodes:manage` capability,
  preserving admin management access while denying normal operators.
- Converted the Push subscription route from authenticated-only access to the `push:subscribe`
  capability, preserving root/admin/operator/viewer/device access while denying unknown roles.
- Converted the debug-only agent trace diagnostics route from root-only access to the
  `diagnostics:read` capability, preserving admin read access while denying normal operators.
- Converted Slock linker management routes from root-only access to the `linkers:manage`
  capability, preserving admin linker management access while denying normal operators.
- Converted agent runtime-inspection read routes from authenticated-only access to the
  `runtime:inspect` capability, preserving viewer read access while denying unknown roles.
- Converted agent runtime-operation routes from authenticated-only access to the `runtime:operate`
  capability, preserving operator start/stop/input/time-trigger/cancel access while denying unknown
  roles.
- Converted agent management routes from authenticated-only access to the `agents:manage`
  capability, including local agent creation, delete, code-mode/config/profile updates, ACP config
  updates, and agent-scoped object upload session routes.
- Converted ACP permission response from authenticated-only access to `runtime:operate`.
- Converted Team definition and channel management routes from authenticated-only access to the
  `teams:manage` capability, covering create/update/delete team plus create/delete channel while
  preserving owner checks after capability authorization.
- Converted Team read-only definition/runtime/channel routes from authenticated-only access to the
  `runtime:inspect` capability, covering prompt defaults, list/get team, get runtime, and list
  channels while preserving owner visibility checks.
- Converted Team runtime operation routes from authenticated-only access to the `runtime:operate`
  capability, covering start/stop team, force a member session, create/cancel/resume/restart run,
  and run context flush while preserving owner/run access checks.
- Converted Team detail read routes from authenticated-only access to the `runtime:inspect`
  capability, covering shared-thread reads, task lists/details/messages, team message search,
  run lists/details/snapshots/events/steps, and run inbox reads while preserving team, task, and run
  visibility checks.
- Converted Team and Team-task upload routes from authenticated-only access to the `teams:manage`
  capability, including inline uploads, upload sessions, direct writes, multipart sessions,
  part uploads, completion, cancellation, and abort paths while preserving Team/task owner-scope
  derivation before metadata publication.
- Converted Team run step-operation routes from authenticated-only access to the `runtime:operate`
  capability, covering submit, start, complete, fail, input-required, and resume step operations
  while preserving run ownership and step-in-run checks.
- Converted Team mailbox-operation routes from authenticated-only access to the `runtime:operate`
  capability, covering run message send plus inbox ack, triage, escalate, transfer, and takeover
  operations while preserving run/member access checks.
- Converted the remaining Team mutation routes from authenticated-only access to explicit
  capability gates: shared-thread ensure, channel-message task creation, and task update use
  `teams:manage`; task conversation send, thread reply, and task run preview use `runtime:operate`.
  Existing agent-only task update restrictions and existing channel-message-to-task behavior are
  preserved.
- Added `push_subscriptions` to the Team API reduced schema fixture so Push API route tests exercise
  the real persistence path, and added the optional `agents.target_node_id` column required by the
  diagnostics collector.
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
cargo test agent_node_routes_require_nodes_manage_capability --locked
cargo test get_agent_node_bootstrap_requires_nodes_manage_capability --locked
cargo test get_agent_node_bootstrap_returns_nodes_manage_join_info --locked
cargo test subscribe_requires_push_subscribe_capability --locked
cargo test agent_trace_requires_diagnostics_read_capability --locked
cargo test slock_linker_routes_require_linkers_manage_and_do_not_expose_secrets --locked
cargo test agent_runtime_inspect_routes_require_runtime_inspect_capability --locked
cargo test agent_runtime_operate_routes_require_runtime_operate_capability --locked
cargo test agent_management_routes_require_agents_manage_capability --locked
cargo test agent_upload_routes_publish_agent_scoped_metadata --locked
cargo test team_management_routes_require_teams_manage_capability --locked
cargo test team_runtime_inspect_routes_require_runtime_inspect_capability --locked
cargo test team_runtime_detail_read_routes_require_runtime_inspect_capability --locked
cargo test team_runtime_operate_routes_require_runtime_operate_capability --locked
cargo test teams_router_accepts_team_upload_route --locked
cargo test team_run_step_operation_routes_require_runtime_operate_capability --locked
cargo test team_mailbox_operation_routes_require_runtime_operate_capability --locked
cargo test team_mutation_routes_require_explicit_capabilities --locked
cargo test set_runtime_profile_route_updates_agent_config --locked
cargo test respond_permission_route_rejects_permission_from_other_agent --locked
cargo test api_code_does_not_bypass_capability_authz_for_human_roles --locked
```

## Follow-Ups

- Continue replacing legacy authenticated-only guards outside the migrated route clusters when they
  are discovered; `src/api/teams.rs` no longer has authenticated-only route handlers.
