# RBAC Multi-User Specification

## Problem

AgentHub currently treats all authenticated users as equivalent. The `role` field on users is a
flat string (`root` or absent) that only gates two operations: agent node management and agent
creation on a specific node. Every authenticated user can see, modify, start, stop, and delete any
agent or team in the system.

This is not tenable beyond single-user homelab use:

- there is no way to grant a teammate access to only their team without also exposing all other
  teams and agents;
- there is no way to prevent accidental (or malicious) deletion of another user's agents;
- the only authorization boundary is "logged in or not";
- Team members have per-Team roles (`coordinator`, `worker`) but those are runtime semantic labels,
  not access-control roles.

We need a proper role-based access control model that supports multiple human users sharing one
AgentHub instance, without introducing multi-tenancy (tenant isolation, separate DBs, org
hierarchies). This is a **single-instance, multi-user** model.

## Scope

- User identity and authentication hardening.
- Role definitions and permission model.
- Per-resource (Agent, Team, Node) ownership and access control.
- API authorization middleware gating all mutable endpoints.
- Frontend UI gating for role-restricted actions.
- Default migration path from the current flat model.
- Audit logging for role changes and destructive actions.

## Non-Goals

- Multi-tenancy: no workspace/org isolation, no tenant-scoped data partitioning, no per-tenant
  billing.
- Fine-grained attribute-based access control (ABAC).
- OIDC / OAuth2 / external identity provider integration (this is a future add-on, not v1).
- Row-level security inside Agent workspaces (file permissions are out of scope).
- Replacing Team member roles (`coordinator`, `worker`) with RBAC roles — those remain runtime
  semantic roles.

## Threat Model

The primary threats this spec addresses:

| Threat | Mitigation |
|--------|------------|
| Authenticated user deletes another user's agent | Resource-level ownership + RBAC |
| Authenticated user starts/stops another user's agent | Same |
| Authenticated user reads another user's Team conversation | Team access control |
| Authenticated user joins a node that hosts other users' agents | Node-level admin gate |
| Privilege escalation via API without frontend | Server-side enforcement, not UI-only |
| Token reuse / session hijacking | Token scoping to user, session revocation |

## Architecture

### 1) User Model

Users are the human actors. Each user has exactly one role.

```
User {
  id: UUID,
  username: String (unique),
  display_name: String,
  role: "admin" | "member",
  password_hash: Option<String>,
  passkey_credential_id: Option<String>,
  created_at: Timestamp,
  updated_at: Timestamp,
  disabled_at: Option<Timestamp>,
}
```

**Roles:**

| Role | Description |
|------|-------------|
| `admin` | Full access. Create/delete users, manage nodes, all agents and teams. Equivalent to current `root`. |
| `member` | Access only to resources they own or are a member of. Cannot manage nodes or users. |

**Constraints:**

- The **first user** created (bootstrap) is always `admin`.
- There must always be at least one `admin` user. Deleting the last admin is rejected.
- `disabled_at` is a soft-delete: the user cannot authenticate, but their resource ownership
  records are preserved.
- Username is the human-facing identifier, independent of the UUID primary key.

### 2) Resource Ownership Model

Every resource has an `owner_user_id`. Additionally, Teams have a `members` list.

```
Agent {
  ...
  owner_user_id: UUID (FK → users.id),
}

Team {
  ...
  owner_user_id: UUID (FK → users.id),
}

TeamMember {
  team_id: UUID,
  user_id: UUID,
}
```

**Ownership rules:**

- The user who creates a resource is its owner.
- Ownership can be transferred by an admin or the current owner.
- `admin` users implicitly have access to all resources (but do not become the owner).

**Team membership rules:**

- A Team has one owner plus zero or more members.
- Team members can view and interact with the Team's conversation, runs, and tasks.
- Only the owner or an admin can delete the Team or manage its membership.
- The Team's agent members (coordinator, workers) are AI agents, not human users. Their
  authentication is via internal gRPC tokens, not user sessions.

### 3) Permission Matrix

| Action | admin | member (owner) | member (team member) | member (unrelated) |
|--------|-------|----------------|----------------------|--------------------|
| Create agent | ✅ | ✅ | - | ✅ |
| View own agent | ✅ | ✅ | - | - |
| View any agent | ✅ | - | - | - |
| Start/stop own agent | ✅ | ✅ | - | - |
| Start/stop any agent | ✅ | - | - | - |
| Delete own agent | ✅ | ✅ | - | - |
| Delete any agent | ✅ | - | - | - |
| Create team | ✅ | ✅ | - | ✅ |
| View own team | ✅ | ✅ | ✅ | - |
| View any team | ✅ | - | - | - |
| Manage team membership | ✅ | ✅ | - | - |
| Delete team | ✅ | ✅ | - | - |
| Manage nodes | ✅ | - | - | - |
| Manage users | ✅ | - | - | - |
| View admin page | ✅ | - | - | - |

**Frontend gating:**

- The `Admin` route is only accessible to `admin` users.
- Team/Agent lists are filtered to show only resources the user can access.
- Destructive action buttons are hidden (not just disabled) when the user lacks permission.

### 4) API Authorization Middleware

All mutable API endpoints must validate permissions server-side.

**Pattern:**

```
fn require_resource_owner(
    user: &UserRecord,
    resource_owner_id: &str,
) -> Result<(), ApiError> {
    if user.role == "admin" || user.id == resource_owner_id {
        Ok(())
    } else {
        Err(ApiError::forbidden("you do not have access to this resource"))
    }
}
```

**Changes to existing endpoints:**

| Endpoint | Current | Required |
|----------|---------|----------|
| `POST /api/agents` | Any authenticated | Any authenticated (ownership auto-set) |
| `GET /api/agents/:id` | Any | Owner or admin |
| `POST /api/agents/:id/input` | Any | Owner or admin |
| `DELETE /api/agents/:id` | Any | Owner or admin |
| `POST /api/agents/:id/start` | Any | Owner or admin |
| `POST /api/agents/:id/stop` | Any | Owner or admin |
| `GET /api/teams/:id` | Any | Owner, member, or admin |
| `DELETE /api/teams/:id` | Any | Owner or admin |
| `POST /api/teams/:id/runtime` | Any | Owner, member, or admin |
| `POST /api/teams/:id/start` | Any | Owner, member, or admin |
| `POST /api/teams/:id/stop` | Any | Owner, member, or admin |
| `POST /api/agent_nodes` | `root` only | `admin` only |
| `GET /api/admin/*` | Any | `admin` only |

**Read-only endpoints** (GET list, status, etc.) should also filter to user-visible resources
rather than returning all resources to every authenticated user.

### 5) Session And Token Security

Current state: a single bearer token per session, stored in `auth_sessions`. Tokens are not scoped
to a user — the user is looked up from the token at validation time.

**Improvements:**

- Session tokens must be associated with a specific `user_id`.
- On login, issue a new session token. On logout, invalidate it.
- `GET /api/auth/sessions` returns the current user's active sessions.
- `DELETE /api/auth/sessions/:id` allows a user to revoke a specific session.
- Admin can view and revoke any user's sessions via `GET /api/admin/sessions`.
- Token validation should be constant-time to avoid timing side-channels.
- CSRF: existing token-in-header pattern is sufficient for an SPA; no cookies needed.

### 6) Migration Path

The current DB has users with optional `role` and no resource ownership.

**Phase 1: Schema migration (non-destructive)**

```sql
ALTER TABLE users ADD COLUMN disabled_at TEXT;
ALTER TABLE agents ADD COLUMN owner_user_id TEXT;
ALTER TABLE teams ADD COLUMN owner_user_id TEXT;
CREATE TABLE team_members (
    team_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    PRIMARY KEY (team_id, user_id)
);
CREATE INDEX idx_agents_owner ON agents(owner_user_id);
CREATE INDEX idx_teams_owner ON teams(owner_user_id);
CREATE INDEX idx_team_members_user ON team_members(user_id);
```

**Phase 2: Data backfill**

- All existing users with `role = 'root'` get `role = 'admin'`.
- All other existing users get `role = 'member'`.
- All existing agents and teams get `owner_user_id = <first admin user id>`.
- The first admin user is added as a member of all existing teams.

**Phase 3: Enforce RBAC at API boundary**

- Activate authorization middleware on all mutable endpoints.
- Frontend gates activate.

**Phase 4: Add user management UI**

- Admin page: list users, create user, disable user, change role.
- Profile page: change display name, view sessions, revoke sessions.

### 7) Audit Logging

| Event | Fields |
|-------|--------|
| `user.created` | actor_id, target_user_id, role |
| `user.role_changed` | actor_id, target_user_id, old_role, new_role |
| `user.disabled` | actor_id, target_user_id |
| `user.enabled` | actor_id, target_user_id |
| `resource.deleted` | actor_id, resource_type, resource_id, resource_name |
| `auth.login` | user_id, method (passkey/password) |
| `auth.login_failed` | username, method, reason |
| `auth.session_revoked` | actor_id, target_user_id, session_id |

Audit events are stored in SQLite and exposed to admin users via the admin page.

### 8) Frontend Changes

- `AuthState` type gains `user_id`, `username`, and `role` fields.
- All list views (agents, teams) filter server-side by user visibility.
- The `Admin` route checks `role === "admin"` before mounting.
- Delete/start/stop buttons are conditionally rendered.
- New "Users" tab on the Admin page for user management.
- Profile dropdown shows current user info and logout option.
- Error messages for 403 responses should be user-friendly ("You don't have access to this team").

### 9) Security Review Checklist

- [ ] All mutable API endpoints have server-side authorization checks.
- [ ] Read endpoints filter results by user visibility.
- [ ] Token validation is constant-time.
- [ ] Session tokens are scoped to `user_id`.
- [ ] Logout invalidates the session server-side.
- [ ] Cannot delete the last admin user.
- [ ] Cannot change your own role (prevents self-demotion attack).
- [ ] `owner_user_id` cannot be changed by non-admin, non-owner users.
- [ ] Team member list cannot be modified by non-owner, non-admin users.
- [ ] Audit log is append-only from the application layer.
- [ ] Database migrations are backward-compatible.

## Rollout Phases

### Phase 1: Schema + Backfill (this PR)

- DB migration to add `owner_user_id`, `team_members`, `disabled_at`.
- Backfill script assigns existing resources to admin.
- No authorization enforcement yet — purely additive.

### Phase 2: API Authorization

- Add `require_resource_owner` and `require_admin` middleware helpers.
- Apply to all CRUD endpoints for agents and teams.
- Apply to node management endpoints.
- Add user-scoped session tokens.

### Phase 3: Frontend Gating

- Filter agent/team lists server-side by visibility.
- Conditionally render action buttons.
- Gate admin route.

### Phase 4: User Management

- Admin user CRUD UI.
- Profile / session management.
- Audit log viewer.

## Validation Matrix

- Rust unit tests for `require_resource_owner` edge cases.
- Integration tests for each endpoint with different roles.
- Frontend unit tests for conditional rendering.
- E2E: admin creates user, user logs in, user can only see own resources.
- E2E: member cannot access admin route.
- E2E: owner cannot delete another user's agent.
- Manual: passkey + password login flows with different roles.
