# RBAC Multi-User Specification

## Problem

AgentHub currently has two roles: `root` (superuser) and `device` (node join). There is no
graduated access control for regular human users. Every authenticated user can see, modify, start,
stop, and delete any agent or team.

This is not tenable beyond single-user homelab use. We need a proper role hierarchy that supports
multiple human users with different privilege levels, without introducing multi-tenancy.

## Scope

- Four-tier human role hierarchy: root → maintainer → member → visitor.
- Keep existing `device` role for node registration (unchanged).
- Per-resource (Agent, Team, Node) ownership and access control.
- API authorization middleware gating all endpoints.
- Frontend UI gating for role-restricted actions.
- Audit logging for role changes and destructive actions.

## Non-Goals

- Multi-tenancy (no org isolation, no per-tenant billing).
- OIDC / OAuth2 / external identity provider integration.
- Fine-grained ABAC or per-file permissions inside workspaces.
- Replacing Team agent roles (`coordinator`, `worker`) — those remain runtime semantic labels.

## Threat Model

| Threat | Mitigation |
|--------|------------|
| Unauthorized agent deletion / start / stop | Resource ownership + role hierarchy |
| Unauthorized Team access | Team membership + role gate |
| Privilege escalation via API | Server-side middleware, not UI-only |
| Token reuse / hijacking | Session scoping to user, revocation |
| Visitor modifying resources | Write operations gated at API layer |

## Architecture

### 1) Role Hierarchy

```
root (管理员)
 ├── maintainer (维护员)
 │    ├── member (员工)
 │    └── visitor (访客)
 └── device (节点注册, system role, no WebUI login)
```

Roles are hierarchical: a higher role includes all permissions of lower roles.

| Role | 中文 | Description |
|------|------|-------------|
| `root` | 管理员 | Superuser. Full access. Manage nodes, users, all resources. Create/delete other roots. |
| `maintainer` | 维护员 | Team/resource manager. Create/manage agents and teams. Manage team membership. Cannot manage nodes or users. |
| `member` | 员工 | Regular user. Create agents and teams. Full access to own resources. Read-only access to teams they are invited to. |
| `visitor` | 访客 | Read-only user. Cannot create resources. Can only view teams they're explicitly invited to. |
| `device` | 节点 | System role for `agenthub join`. No WebUI login. Unchanged from current behavior. |

**Constraints:**

- The bootstrap user (first created) is always `root`.
- There must always be at least one `root`. Deleting the last root is rejected.
- A user cannot change their own role (prevents self-promotion and self-demotion).
- `device` users cannot authenticate via WebUI.
- `visitor` users cannot create agents, teams, or send messages.

### 2) Permission Matrix

| Action | root | maintainer | member | visitor |
|--------|------|------------|--------|---------|
| **User management** | | | | |
| Create/delete users | ✅ | - | - | - |
| Change user roles | ✅ | - | - | - |
| View all users | ✅ | ✅ | - | - |
| **Node management** | | | | |
| Register/remove nodes | ✅ | - | - | - |
| View nodes | ✅ | ✅ | ✅ | - |
| **Agent operations** | | | | |
| Create agent | ✅ | ✅ | ✅ | - |
| View own agents (meta + full) | ✅ | ✅ | ✅ | ✅ |
| View any agent (meta only) | ✅ | ✅ | ✅ | ✅ |
| Send input to own agent | ✅ | ✅ | ✅ | - |
| Send input to any agent | ✅ | ✅ | - | - |
| Start/stop own agent | ✅ | ✅ | ✅ | - |
| Start/stop any agent | ✅ | ✅ | - | - |
| Delete own agent | ✅ | ✅ | ✅ | - |
| Delete any agent | ✅ | ✅ | - | - |
| **Team operations** | | | | |
| Create team | ✅ | ✅ | ✅ | - |
| View own teams | ✅ | ✅ | ✅ | ✅ |
| View any team | ✅ | ✅ | - | - |
| Invite members to own team | ✅ | ✅ | ✅ (owner) | - |
| Invite members to any team | ✅ | ✅ | - | - |
| Send conversation messages | ✅ | ✅ | ✅ (own/member-of) | - |
| Delete own team | ✅ | ✅ | ✅ (owner) | - |
| Delete any team | ✅ | ✅ | - | - |
| Start/stop team runtime | ✅ | ✅ | ✅ (own/member-of) | - |
| **Admin page** | | | | |
| View admin dashboard | ✅ | - | - | - |
| View system config | ✅ | - | - | - |

**Key rules:**

- `root` can do everything, everywhere.
- `maintainer` can manage all agents and teams (create, delete, start, stop) but cannot touch nodes or users.
- `member` can create agents and teams. Can only manage (start, stop, delete, send input to)
  agents they created themselves. For teams they don't own, they can view and participate in
  conversations (if invited), but cannot manage membership or delete.
- `visitor` is strictly read-only. **Cannot create agents. Cannot manage any agents.**
  Can view all agent metadata. Cannot send input to any agent. Cannot create, modify, or delete
  anything. Cannot send messages.

### 3) Resource Ownership

Every Agent and Team has an `owner_user_id`.

```sql
ALTER TABLE agents ADD COLUMN owner_user_id TEXT REFERENCES users(id);
ALTER TABLE teams ADD COLUMN owner_user_id TEXT REFERENCES users(id);
CREATE TABLE team_members (
    team_id TEXT NOT NULL REFERENCES teams(id),
    user_id TEXT NOT NULL REFERENCES users(id),
    PRIMARY KEY (team_id, user_id)
);
```

**Ownership on create:**
- `POST /api/agents` → sets `owner_user_id = current user.id`
- `POST /api/teams` → sets `owner_user_id = current user.id`

### 4) API Authorization Middleware

Existing middleware (`api/authz.rs:27`):

```rust
pub fn require_admin(user: &UserRecord) -> Result<(), ApiError> {
    if user.role != "root" { return Err(ApiError::unauthorized("root role required")); }
    Ok(())
}
```

**New middleware:**

```rust
pub fn require_maintainer(user: &UserRecord) -> Result<(), ApiError> {
    if user.role != "root" && user.role != "maintainer" {
        return Err(ApiError::unauthorized("maintainer role or above required"));
    }
    Ok(())
}

pub fn require_resource_owner(user: &UserRecord, owner_id: &str) -> Result<(), ApiError> {
    if user.role == "root" || user.role == "maintainer" || user.id == owner_id {
        Ok(())
    } else {
        Err(ApiError::forbidden("you do not have access to this resource"))
    }
}

pub fn require_not_visitor(user: &UserRecord) -> Result<(), ApiError> {
    if user.role == "visitor" {
        return Err(ApiError::forbidden("visitors cannot perform write operations"));
    }
    Ok(())
}
```

**Endpoint gating:**

| Endpoint | Required role |
|----------|---------------|
| `POST /api/agents` | `require_not_visitor` |
| `GET /api/agents` | filtered by visibility |
| `GET /api/agents/:id` | `require_resource_owner` (root/maintainer see all) |
| `POST /api/agents/:id/start` | `require_resource_owner` |
| `POST /api/agents/:id/stop` | `require_resource_owner` |
| `DELETE /api/agents/:id` | `require_resource_owner` |
| `POST /api/teams` | `require_not_visitor` |
| `GET /api/teams` | filtered by user visibility |
| `GET /api/teams/:id` | owner/member/root/maintainer |
| `DELETE /api/teams/:id` | `require_resource_owner` + `require_maintainer` for non-owned |
| `POST /api/teams/:id/start` | owner/member/root/maintainer |
| `POST /api/teams/:id/stop` | owner/member/root/maintainer |
| `POST /api/agent_nodes` | `require_admin` (root only) |
| `GET /api/admin/*` | `require_admin` (root only) |
| `POST /api/auth/register` (with role=root) | `require_admin` |
| `POST /api/auth/register` (with role=maintainer) | `require_admin` |

### 5) Visitor-Specific Guards

- `GET` endpoints for agents/teams return metadata for all resources to visitors (no filtering by
  ownership). Visitors can browse and discover what's running.
- `require_not_visitor` middleware is applied to ALL write endpoints (POST, PUT, DELETE, PATCH).
- Visitors cannot send Team conversation messages.
- Visitors cannot create agents.
- Visitors cannot start/stop runs.
- Visitors cannot send input to any agent (even ones they own).

### 6) Session And Token Security

- Tokens scoped to `user_id` (already tracked via `validate_session`).
- Logout invalidates session server-side.
- `GET /api/auth/sessions` → list current user's sessions.
- `DELETE /api/auth/sessions/:id` → revoke a session.
- `root` can view/revoke all sessions via admin API.

### 7) Migration Path

**Phase 1: Schema + Backfill**

- Add `owner_user_id` to agents, teams (nullable initially).
- Add `team_members` table.
- Add `disabled_at` to users.
- Add `maintainer`, `member`, `visitor` as valid role values in registration.
- Backfill: all existing users keep `root`. All existing agents/teams get `owner_user_id` from
  first root.

**Phase 2: API Authorization**

- Apply role middleware to all endpoints.
- Filter list endpoints by user visibility.

**Phase 3: Frontend Gating**

- Conditional rendering based on role.
- Admin route for root only.
- Visitor gets simplified read-only UI.

**Phase 4: User Management UI**

- Admin page: list users, create user, change role.
- Self-service: profile, session management.

### 8) Audit Logging

| Event | Fields |
|-------|--------|
| `user.created` | actor_id, target_user_id, role |
| `user.role_changed` | actor_id, target_user_id, old_role, new_role |
| `user.disabled` / `user.enabled` | actor_id, target_user_id |
| `resource.deleted` | actor_id, resource_type, resource_id, resource_name |
| `auth.login` / `auth.login_failed` | user_id, method |

### 9) Security Review Checklist

- [ ] All write endpoints have `require_not_visitor`.
- [ ] Admin endpoints have `require_admin` (root only).
- [ ] Node management is root-only.
- [ ] `require_resource_owner` allows root + maintainer bypass.
- [ ] Visitors cannot send messages or create resources.
- [ ] Cannot delete last root.
- [ ] Cannot self-promote or self-demote.
- [ ] `device` users cannot WebUI login.
- [ ] Audit log is append-only.

## Validation Matrix

- Unit tests for each middleware helper.
- Integration tests for each endpoint × role combination.
- E2E: root creates maintainer → maintainer creates member → member creates agent.
- E2E: visitor cannot create agent, cannot send message.
- E2E: member cannot delete another user's agent.
- E2E: maintainer can delete any agent but cannot access admin page.
