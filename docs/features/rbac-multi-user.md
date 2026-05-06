# RBAC Multi-User Specification

## Problem

AgentHub currently has a minimal role system: `root` for the superuser and `device` for node
join. There is no `member` role for regular human users beyond the first admin. Every
authenticated user can see, modify, start, stop, and delete any agent or team in the system.

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
- New `member` role for regular human users (alongside existing `root` and `device`).
- Per-resource (Agent, Team, Node) ownership and access control.
- API authorization middleware gating all mutable endpoints.
- Frontend UI gating for role-restricted actions.
- Default migration path from the current model.
- Audit logging for role changes and destructive actions.

## Non-Goals

- Multi-tenancy: no workspace/org isolation, no tenant-scoped data partitioning, no per-tenant
  billing.
- Fine-grained attribute-based access control (ABAC).
- OIDC / OAuth2 / external identity provider integration (this is a future add-on, not v1).
- Row-level security inside Agent workspaces (file permissions are out of scope).
- Replacing Team member roles (`coordinator`, `worker`) with RBAC roles — those remain runtime
  semantic roles.
- Changing `root` or `device` role semantics — these are existing and remain stable.

## Threat Model

The primary threats this spec addresses:

| Threat | Mitigation |
|--------|------------|
| Authenticated user deletes another user's agent | Resource-level ownership + role check |
| Authenticated user starts/stops another user's agent | Same |
| Authenticated user reads another user's Team conversation | Team access control |
| Authenticated user joins a node that hosts other users' agents | Node-level `root` gate |
| Privilege escalation via API without frontend | Server-side enforcement, not UI-only |
| Token reuse / session hijacking | Token scoping to user, session revocation |

## Architecture

### 1) User Model

Users are the human actors (plus Node devices). Each user has exactly one role.

**Existing roles (unchanged):**

| Role | Description | Added in |
|------|-------------|----------|
| `root` | Superuser. Full access to all resources. Create/manage nodes, agents, teams, and users. | Existing (`auth/mod.rs`) |
| `device` | Node registration identity. Used by `agenthub join`. Cannot login via WebUI. | Existing (`api/join.rs`) |

**New role (this spec):**

| Role | Description |
|------|-------------|
| `member` | Regular human user. Access only to resources they own or are Team members of. |

**Constraints:**

- The bootstrap user (first user created) is always `root`.
- There must always be at least one `root` user. Deleting the last root is rejected.
- `device` users cannot authenticate via the WebUI (no passkey/password login flow).
- `member` users cannot register passkeys or use the WebUI login if passkey is enabled and they
  have no credential — they must be created by a `root` user.

**DB schema (additive changes):**

```sql
-- Existing table (unchanged columns)
-- CREATE TABLE users (id, username, display_name, role, password_hash, created_at);

-- New columns
ALTER TABLE users ADD COLUMN disabled_at TEXT;  -- soft-disable

-- Existing auth_sessions stays, but session.user_id must be populated
```

**API changes for registration:**

`POST /api/auth/register` already accepts `role` parameter (from `api/auth.rs:106`).
Currently it defaults to `"device"` when `role` is absent. We add `"member"` as a valid role
and require `root` authorization to create another `root` user.

### 2) Resource Ownership Model

Every resource has an `owner_user_id`. Additionally, Teams have a `members` list.

**New DB columns:**

```sql
ALTER TABLE agents ADD COLUMN owner_user_id TEXT REFERENCES users(id);
ALTER TABLE teams ADD COLUMN owner_user_id TEXT REFERENCES users(id);
CREATE TABLE team_members (
    team_id TEXT NOT NULL REFERENCES teams(id),
    user_id TEXT NOT NULL REFERENCES users(id),
    PRIMARY KEY (team_id, user_id)
);
CREATE INDEX idx_agents_owner ON agents(owner_user_id);
CREATE INDEX idx_teams_owner ON teams(owner_user_id);
CREATE INDEX idx_team_members_user ON team_members(user_id);
```

**Ownership rules:**

- The user who creates a resource is its owner.
- Ownership can be transferred by a `root` user.
- `root` users implicitly have access to all resources (but are not the owner).

**Team membership rules:**

- A Team has one owner plus zero or more human members.
- Team members can view and interact with the Team's conversation, runs, and tasks.
- Only the owner or a `root` user can delete the Team or manage its membership.
- The Team's agent members (coordinator, workers) are AI agents, not human users. Their
  authentication is via internal gRPC tokens, not user sessions.

### 3) Permission Matrix

| Action | root | member (owner) | member (team member) | member (unrelated) |
|--------|------|----------------|----------------------|--------------------|
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

- The `Admin` route is only accessible to `root` users.
- Team/Agent lists are filtered to show only resources the user can access.
- Destructive action buttons are hidden (not just disabled) when the user lacks permission.

### 4) API Authorization Middleware

All mutable API endpoints must validate permissions server-side.

Existing middleware in `api/authz.rs` already has `require_admin`:

```rust
// Existing (api/authz.rs:27)
pub fn require_admin(user: &UserRecord) -> Result<(), ApiError> {
    if user.role != "root" {
        return Err(ApiError::unauthorized("root role required"));
    }
    Ok(())
}
```

**New middleware to add:**

```rust
pub fn require_resource_owner(
    user: &UserRecord,
    resource_owner_id: &str,
) -> Result<(), ApiError> {
    if user.role == "root" || user.id == resource_owner_id {
        Ok(())
    } else {
        Err(ApiError::forbidden("you do not have access to this resource"))
    }
}

pub fn require_team_access(
    user: &UserRecord,
    team_owner_id: &str,
    team_member_ids: &[String],
) -> Result<(), ApiError> {
    if user.role == "root" || user.id == team_owner_id || team_member_ids.contains(&user.id) {
        Ok(())
    } else {
        Err(ApiError::forbidden("you do not have access to this team"))
    }
}
```

**Changes to existing endpoints:**

| Endpoint | Current | Required |
|----------|---------|----------|
| `POST /api/agents` | `require_user` | `require_user` (ownership auto-set) |
| `GET /api/agents/:id` | `require_user` | Owner or root |
| `POST /api/agents/:id/input` | `require_user` | Owner or root |
| `DELETE /api/agents/:id` | `require_user` | Owner or root |
| `POST /api/agents/:id/start` | `require_user` | Owner or root |
| `POST /api/agents/:id/stop` | `require_user` | Owner or root |
| `GET /api/teams/:id` | `require_user` | Owner, member, or root |
| `DELETE /api/teams/:id` | `require_user` | Owner or root |
| `POST /api/teams/:id/runtime` | `require_user` | Owner, member, or root |
| `POST /api/teams/:id/start` | `require_user` | Owner, member, or root |
| `POST /api/teams/:id/stop` | `require_user` | Owner, member, or root |
| `POST /api/agent_nodes` | `require_admin` (root) | `require_admin` (unchanged) |
| `GET /api/admin/*` | `require_user` | `require_admin` (root) |

**Read-only endpoints** (GET list, status, etc.) should also filter to user-visible resources.

### 5) Session And Token Security

Current state: bearer token in `auth_sessions`. Tokens are validated via `AuthService::validate_session`.

**Improvements:**

- Session tokens must be associated with a specific `user_id` (already tracked via `validate_session` → `UserRecord`).
- On logout, invalidate the session server-side.
- `GET /api/auth/sessions` returns the current user's active sessions.
- `DELETE /api/auth/sessions/:id` allows a user to revoke a specific session.
- `root` can view and revoke any user's sessions via `GET /api/admin/sessions`.
- Token validation should be constant-time to avoid timing side-channels.

### 6) Migration Path

**Phase 1: Schema migration (non-destructive)**

- Add `owner_user_id` to agents and teams tables.
- Add `team_members` table.
- Add `disabled_at` to users table.
- Add `member` as valid role in registration endpoint.
- All existing agents/teams get `owner_user_id = <first root user id>`.

**Phase 2: API Authorization**

- Apply `require_resource_owner` / `require_team_access` to all CRUD endpoints.
- Apply `require_admin` to admin and node endpoints.
- Add user-scoped session listing and revocation.

**Phase 3: Frontend Gating**

- Filter agent/team lists by user visibility.
- Conditionally render action buttons.
- Hide admin route from non-root users.

**Phase 4: User Management**

- Admin user CRUD UI.
- Profile / session management.
- Audit log viewer.

### 7) Audit Logging

| Event | Fields |
|-------|--------|
| `user.created` | actor_id, target_user_id, role |
| `user.role_changed` | actor_id, target_user_id, old_role, new_role |
| `user.disabled` | actor_id, target_user_id |
| `resource.deleted` | actor_id, resource_type, resource_id, resource_name |
| `auth.login` | user_id, method (passkey/password) |
| `auth.login_failed` | username, method, reason |
| `auth.session_revoked` | actor_id, target_user_id, session_id |

### 8) Security Review Checklist

- [ ] All mutable API endpoints have server-side authorization checks.
- [ ] Read endpoints filter results by user visibility.
- [ ] Token validation is constant-time.
- [ ] Session tokens are scoped to `user_id`.
- [ ] Logout invalidates the session server-side.
- [ ] Cannot delete the last `root` user.
- [ ] Cannot change your own role (prevents self-demotion attack).
- [ ] `owner_user_id` cannot be changed by non-root, non-owner users.
- [ ] Team member list cannot be modified by non-owner, non-root users.
- [ ] `device` users cannot login via WebUI.
- [ ] `member` users cannot create `root` users.
- [ ] Audit log is append-only from the application layer.
- [ ] Database migrations are backward-compatible.

## Validation Matrix

- Rust unit tests for `require_resource_owner` and `require_team_access` edge cases.
- Integration tests for each endpoint with different roles.
- Frontend unit tests for conditional rendering.
- E2E: root creates member, member logs in, member can only see own resources.
- E2E: member cannot access admin route.
- E2E: member cannot delete another user's agent.
- E2E: device user cannot login via WebUI.
- Manual: passkey + password login flows with different roles.
