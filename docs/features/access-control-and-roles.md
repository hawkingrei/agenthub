# Access Control And Roles

## Problem

AgentHub currently has two useful but disconnected authorization models:

- Browser/API auth is coarse-grained: authenticated `user` plus root-only admin gates.
- Internal Team/runtime auth is action-based: internal tokens carry role, run scope, actor scope, and
  explicit permissions.

That split is workable for a local single-operator control plane, but it becomes too blunt for
shared Team and node operation. Product surfaces need named user roles and capabilities without
spreading direct `role == ...` checks across routes and services.

## Scope

- Human-facing user roles and HTTP/API capabilities.
- Team/member runtime roles as a separate execution model.
- Internal gRPC/runtime token permissions as a separate machine-auth model.
- A migration path from root-only gates to capability gates.
- Regression guardrails that prevent new authorization bypasses.

## Non-Goals

- Replacing existing authentication, sessions, passkeys, or device login.
- Implementing multi-tenant organizations in this spec.
- Changing Team coordinator/worker execution semantics.
- Replacing internal gRPC action permissions.
- Adding UI role-management workflows in the first implementation slice.

## Architecture

### 1) Identity Layers

Use separate identity layers instead of one overloaded role string:

| Layer | Principal | Role source | Capability source | Examples |
| --- | --- | --- | --- | --- |
| Human API | authenticated user | `users.role` | user capability matrix | configure instance, manage agents, manage nodes |
| Device API | authenticated device user | `users.role = device` plus active device row | limited device capability matrix | push subscription, device heartbeat |
| Team runtime | Team member | `spec.members[].role` | Team role/runtime policy | coordinator, worker |
| Internal runtime | internal token principal | JWT `role` | token permission claims plus role defaults | Team actor CLI, node relay |

The same account can participate in more than one layer, but authorization decisions must name the
layer they are evaluating.

### 2) User Role Model

The v1 user roles are:

| Role | Intent |
| --- | --- |
| `root` | Instance owner and break-glass administrator. Can configure security-critical instance state. |
| `admin` | Day-to-day operator. Can manage normal agents, Teams, and nodes, but cannot change root-only security settings. |
| `operator` | Power user for runtime operation. Can create/start/stop agents and inspect Team output, but cannot alter instance-wide config. |
| `viewer` | Read-only human user. Can inspect visible agents, Teams, and logs exposed by policy. |
| `device` | Device-scoped principal. Exists for device workflows and should not inherit human admin capabilities. |

Compatibility path:

- Existing `root` users retain all current authority.
- Existing non-root human users should map to `operator` when the migration introduces the new roles.
- Existing `device` users keep device-only behavior.

### 3) Capability Matrix

Capabilities are stable action names; routes and services should ask for a capability, not inspect a
role directly.

| Capability | root | admin | operator | viewer | device |
| --- | --- | --- | --- | --- | --- |
| `instance:configure` | yes | no | no | no | no |
| `users:manage` | yes | no | no | no | no |
| `auth:manage` | yes | no | no | no | no |
| `agents:manage` | yes | yes | yes | no | no |
| `teams:manage` | yes | yes | yes | no | no |
| `nodes:manage` | yes | yes | no | no | no |
| `linkers:manage` | yes | yes | no | no | no |
| `runtime:operate` | yes | yes | yes | no | no |
| `runtime:inspect` | yes | yes | yes | yes | no |
| `diagnostics:read` | yes | yes | no | no | no |
| `push:subscribe` | yes | yes | yes | yes | yes |

`root` remains the only role that can change security-sensitive instance settings such as passkey
mode, root lifecycle, root-owned safe paths, and user role assignments.

### 4) Authorization Entry Points

HTTP/API authorization should converge on a small entry-point module:

- `require_user`: authenticate only.
- `require_capability`: authenticate and check one user capability.
- `require_any_capability`: authenticate and check at least one capability.
- `require_root`: compatibility wrapper for `require_capability("instance:configure")` or a
  dedicated break-glass check where root-only semantics are intentional.

New code should not add direct user-role checks outside the canonical authz module and auth domain
tests.

### 5) Resource Boundary Contract

Capability checks answer "may this principal attempt this kind of action." Resource-scoped checks
must still happen after capability checks:

- Agent and Team operations must still verify resource ownership/visibility.
- Run-scoped APIs must still verify run/team/user boundaries.
- Node operations must still verify main/node topology and internal gRPC scope.
- Team runtime actions must still enforce internal token actor/run scope.

Capabilities do not replace ownership checks.

### 6) Runtime Role Separation

Do not merge human user roles with Team member roles:

- `coordinator` and `worker` are Team execution roles, not human authorization roles.
- Internal token roles (`coordinator`, `worker`, `orchestrator`) remain runtime transport roles.
- Human capabilities may allow starting or inspecting a Team, but they do not let a human impersonate
  a Team member actor.

### 7) Boundary Tests

Authorization changes should include three guardrails:

1. Matrix tests
   - Assert every role/capability pair explicitly.
   - Include null/unknown role denial.
2. Route behavior tests
   - Prove a lower role is denied for a protected route.
   - Prove the nearest allowed role succeeds.
3. Bypass guard
   - Add a static or focused test that fails when new route/service code performs direct role checks
     outside the canonical authz module.

## Contracts

### 1) Capability Names Are Product Contracts

Capability names must be stable, readable, and action-oriented. Avoid naming capabilities after
routes or implementation modules.

### 2) Deny By Default

Unknown roles and unknown capabilities deny. Missing sessions deny. Device users deny every human
capability unless explicitly granted by the device matrix.

### 3) Root Is Not The Default Implementation Shortcut

Root-only should mean security-critical or break-glass. Normal operation should move toward
capability gates so non-root operators can safely run agents and Teams.

### 4) Compatibility Wrappers Must Be Temporary

Existing `require_root` routes may remain during migration, but new routes should prefer
capability-oriented helpers. When a root-only route is intentionally root-only, document why in the
route or owning feature spec.

### 5) Audit Sensitive Mutations

Role changes, auth configuration changes, linker credential updates, node credential changes, and
diagnostic export actions should write audit records with principal id, capability, target, and
result.

## Validation Matrix

| Change | Required validation |
| --- | --- |
| Add or change a user role | Matrix tests for all capabilities and null/unknown denial. |
| Add or change a capability | Matrix tests plus docs update in this spec. |
| Convert a root-only route to capability auth | Focused route test for denied lower role and allowed nearest role. |
| Add a new protected route | Route test plus bypass guard coverage. |
| Change internal runtime permissions | Existing internal authz tests plus role-default permission tests. |
| Change Team member roles | Team role tests; do not reuse human role capability tests. |

## Operational Notes

- Start with domain-only helpers and tests before migrating routes.
- Convert routes by capability cluster, not file-by-file churn:
  1. runtime inspect/operate;
  2. agent/team management;
  3. node management;
  4. linker management;
  5. root-only security settings.
- Keep error messages precise but avoid resource existence leaks when the user lacks the surrounding
  resource boundary.
- Keep UI gates advisory; backend authorization remains authoritative.

## Open Risks

- Existing root-only routes mix true security settings with normal operator actions; migration needs
  careful classification.
- Adding roles without UI management can make tests pass while operators still cannot administer
  users conveniently.
- Bypass guard patterns can produce false positives; keep the baseline small and explicit.
- Role migration must preserve local single-user setups without forcing a disruptive onboarding step.

## Source Journals

- [2026-07-16 Access Control Roles](../journal/2026-07-16-access-control-roles.md)
- [2026-07-21 Linker Capability Gate](../journal/2026-07-21-linker-capability-gate.md)
- [2026-07-21 Agent Inspect Capability Gate](../journal/2026-07-21-agent-inspect-capability-gate.md)
- [2026-07-21 Agent Runtime Capability Gate](../journal/2026-07-21-agent-runtime-capability-gate.md)
- [2026-07-21 Agent Management Capability Gate](../journal/2026-07-21-agent-management-capability-gate.md)
- [2026-07-21 Settings Runtime Defaults Capability Gate](../journal/2026-07-21-settings-runtime-defaults-capability-gate.md)
- [2026-07-21 Team Prompt Defaults Capability Gate](../journal/2026-07-21-team-prompt-defaults-capability-gate.md)
- [2026-07-21 Team Management Capability Gate](../journal/2026-07-21-team-management-capability-gate.md)
- [2026-07-21 Team Runtime Control Capability Gate](../journal/2026-07-21-team-runtime-control-capability-gate.md)
- [2026-07-21 Team Run Step Capability Gate](../journal/2026-07-21-team-run-step-capability-gate.md)
- [2026-07-21 Team Upload Capability Gate](../journal/2026-07-21-team-upload-capability-gate.md)
- [2026-07-21 Team Mailbox Capability Gate](../journal/2026-07-21-team-mailbox-capability-gate.md)
- [2026-07-21 Team Task Capability Gate](../journal/2026-07-21-team-task-capability-gate.md)
- [2026-07-21 Team Thread Reply Capability Gate](../journal/2026-07-21-team-thread-reply-capability-gate.md)
- [2026-07-21 Team Read Preview Capability Gate](../journal/2026-07-21-team-read-preview-capability-gate.md)
