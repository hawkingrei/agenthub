# Slock OAuth Linkers Specification

## Problem

AgentHub needs a stable way to connect external applications such as Slock so
agents can inspect external context without receiving raw third-party tokens or
learning app-specific login rituals.

The Slock integration should follow one login model:

```text
Login with Slock -> callback -> token exchange -> userinfo -> server-side linked account
```

Humans and Slock agents are both Slock principals. AgentHub may display whether
the linked principal is a human or an agent, but the third-party integration must
not split into separate human-login and agent-login protocols.

## Scope

- Add a canonical `Linkers` admin surface for external app connections.
- Define Slock as the first OAuth-like linker provider.
- Store Slock OAuth client configuration server-side:
  - API origin
  - client id
  - client secret
  - return URL
  - requested scopes
- Support one Slock callback-code exchange path for human and agent principals.
- Store a server-side linked account identity snapshot from Slock `userinfo`.
- Provide read-only Slock resource access through AgentHub-controlled tools.
- Keep Slock access tokens and client secrets out of browser JavaScript, agent
  prompts, logs, screenshots, and tool output.

## Non-Goals

- Replacing AgentHub local authentication with Slock login.
- Creating a separate agent-only Slock login route.
- Exposing Slock access tokens to agents.
- Letting agents call Slock internal grant, daemon, or machine APIs directly.
- Building a generic unrestricted HTTP proxy to Slock.
- Depending on undocumented Slock internal database tables or grant records.
- Implementing full OIDC validation such as signed ID tokens and JWKS in the
  first version.
- Syncing all Slock history into AgentHub storage by default.

## Architecture

### 1) Naming Model

`AppConnectorId` identifies a provider adapter and its stable capability
surface. It is not a specific user login.

Examples:

```text
slock
github
linear
```

`AppLinkerId` identifies one configured connection instance inside AgentHub.
It is the handle admins and tools use to address a concrete external app
connection.

Examples:

```text
slock-primary
slock-dev
```

`LinkedPrincipal` identifies the external subject returned by the provider.
For Slock, this is `userinfo.sub` plus `userinfo.type`.

### 2) Data Model

AgentHub stores connector configuration and linked account state separately:

```text
app_linkers
- id
- connector_id
- display_name
- provider
- status
- config_json
- created_by_user_id
- created_at
- updated_at

app_linker_secrets
- linker_id
- client_secret
- access_token
- token_type
- scope
- expires_at
- updated_at

app_linker_principals
- linker_id
- subject
- principal_type
- display_name
- handle
- avatar_url
- server_id
- server_slug
- raw_userinfo_json
- updated_at
```

Secrets remain server-side only. Browser APIs return redacted state such as
`configured`, `connected`, `expires_at`, and the identity snapshot.

### 3) Admin Flow

Only `root` users can configure linkers.

The admin page exposes a `Linkers` section with a Slock provider card:

1. root enters Slock API origin, client id, client secret, return URL, and
   scopes.
2. AgentHub stores the provider config and redacts the secret in responses.
3. root starts a short-lived Slock link attempt, which creates an opaque
   `state` bound to the linker id and initiating root user.
4. root completes Slock login. Slock either redirects to AgentHub's Slock
   callback URL with `code` and `state`, or the root pastes a returned callback
   URL/code into the authenticated Admin form.
5. AgentHub exchanges the code at:

```text
POST {slock_api_origin}/api/oauth/token
Authorization: Basic base64(client_id:client_secret)
Content-Type: application/json
```

6. AgentHub calls:

```text
GET {slock_api_origin}/api/oauth/userinfo
Authorization: Bearer <access_token>
```

7. AgentHub stores the identity snapshot and marks the linker connected.

The same callback handler is used for human and agent Slock principals.

### 4) Runtime Resource Access

Agents query Slock through AgentHub, not directly.

The canonical path is:

```text
agent -> AgentHub tool/CLI -> internal authorization -> linker service -> Slock API
```

Initial Slock read tools:

```text
agenthub actor linker-list
agenthub actor slock-channels --linker-id <id>
agenthub actor slock-channel-messages --linker-id <id> --channel-id <id> --limit <n>
```

The CLI tools may later be wrapped as MCP tools or plugin tools, but the backend
linker service remains the root of trust.

### 5) Slock Resource Boundary

Slock OAuth `userinfo` is the identity source of truth for linked principals.
Channel and message reads require Slock resource APIs and scopes that are
separate from the minimal login contract.

AgentHub must not expose an arbitrary path proxy. Each resource operation needs
a provider adapter method, a scope requirement, and a bounded response shape.

Reference resource methods:

```text
SlockLinker.list_channels(linker_id, limit)
SlockLinker.list_channel_messages(linker_id, channel_id, limit, cursor)
```

If Slock channel/message endpoint paths are not stable yet, the implementation
must keep these methods behind adapter-level tests or feature flags instead of
turning guessed paths into public contracts.

## Contracts

### 1) Slock OAuth Config

```json
{
  "connector_id": "slock",
  "linker_id": "slock-primary",
  "display_name": "Slock",
  "api_origin": "https://api.slock.ai",
  "client_id": "agenthub",
  "return_url": "https://agenthub.example.com/api/linkers/slock/callback",
  "scopes": ["identity", "openid", "profile"]
}
```

`client_secret` is accepted only in write requests and never returned.

### 2) Slock Token Exchange

AgentHub supports two exchange modes:

- authenticated Admin exchange: a root submits the callback code or full
  callback URL to AgentHub from the Admin page.
- redirect callback exchange: Slock redirects to AgentHub with `code` and
  `state`; AgentHub accepts it only when `state` matches a live link attempt.

Request to Slock:

```http
POST /api/oauth/token
Authorization: Basic base64(client_id:client_secret)
Content-Type: application/json
```

```json
{
  "grant_type": "authorization_code",
  "code": "<callback-code>"
}
```

Stored fields:

- access token
- token type
- expiry timestamp
- granted scopes

The callback code is single-use and must not be stored after exchange.
The `state` value is single-use, short-lived, and must be deleted after success
or failure.

### 3) Slock Userinfo Snapshot

Required Slock fields:

```json
{
  "sub": "27a3edb7-4e03-4a42-a61d-63fc04fce62c",
  "type": "agent",
  "client_id": "agenthub",
  "server_id": "bb191bdf-efe0-4733-b30e-cd26bf37d609",
  "preferred_username": "assistant",
  "name": "Claude Assistant"
}
```

AgentHub stores `sub` as the stable external subject. It must not use
`preferred_username` as an immutable identifier.

### 4) Admin API

Reference routes:

```text
GET  /api/admin/linkers
PUT  /api/admin/linkers/slock
POST /api/admin/linkers/slock/link_attempts
POST /api/admin/linkers/slock/exchange
GET  /api/admin/linkers/slock/userinfo
GET  /api/admin/linkers/slock/channels
GET  /api/admin/linkers/slock/channels/:channel_id/messages
```

All `/api/admin/linkers/*` routes require `root`.

The redirect callback route is separate from admin APIs:

```text
GET /api/linkers/slock/callback?code=...&state=...
```

It does not rely on browser cookies or local-storage bearer tokens. It relies on
the opaque server-side `state` created by a root-owned link attempt.

Redacted admin responses use this shape:

```json
{
  "linker_id": "slock-primary",
  "connector_id": "slock",
  "status": "connected",
  "client_id": "agenthub",
  "client_secret_configured": true,
  "principal": {
    "subject": "27a3edb7-4e03-4a42-a61d-63fc04fce62c",
    "type": "agent",
    "display_name": "Claude Assistant",
    "handle": "assistant"
  }
}
```

### 5) Agent Tool Authorization

Agent-side linker reads require an internal permission distinct from Team
mailbox permissions:

```text
app_linker:read
```

Tool calls are read-only in the first version. They may list Slock channels and
read bounded channel message pages. They may not send Slock messages, mutate
Slock settings, or refresh tokens without a root-owned admin flow.

### 6) Audit Contract

AgentHub records audit events for:

- linker config created or updated
- OAuth code exchange completed or failed
- linked principal changed
- token/userinfo refresh failed
- agent tool queried Slock channels
- agent tool queried Slock channel messages

Audit records must not include access tokens, client secrets, callback codes, or
full message payloads.

## Validation Matrix

| Area | Validation |
|---|---|
| Admin config | Root can save Slock config; non-root requests are rejected. |
| Secret redaction | API responses never include `client_secret` or `access_token`. |
| Token exchange | Backend sends Basic auth and JSON grant request to Slock token endpoint. |
| Userinfo mapping | `sub`, `type`, names, server fields, and scopes are stored with correct defaults. |
| Callback parity | Human and agent callback codes use the same exchange handler. |
| Agent tool auth | Tool calls require `app_linker:read` and never expose raw tokens. |
| Resource bounds | Channel/message reads clamp limits and reject empty or malformed channel ids. |
| UI | Admin `Linkers` tab shows configured/connected state and redacted identity details. |
| Browser | Admin page can configure Slock and show connected identity without console errors. |

Focused test targets:

```text
cargo test -p agenthub slock_linker
cargo test -p agenthub admin_linkers
cd web && npm run test -- src/use_app_admin.test.tsx src/pages/admin_page.test.tsx src/pages/admin_page_sections.test.tsx
cd web && npm run lint
cd web && npm exec tsc -- --noEmit
```

## Operational Notes

- Default Slock API origin is `https://api.slock.ai`.
- Operators should configure a return URL under AgentHub's admin API origin.
- Slock `client_secret` must be rotated if it is ever exposed in browser logs,
  screenshots, chat transcripts, or agent output.
- Token expiry should be visible in Admin. A disconnected or expired linker must
  fail closed for agent tools.
- Resource APIs should use small default page sizes. Channel history sync should
  remain explicit and bounded.

## Open Risks

- Slock channel/message endpoint contracts are not defined by the minimal login
  guide. The first implementation needs Slock resource API confirmation before
  exposing stable channel/message methods.
- The first version may store secrets in SQLite without envelope encryption.
  This is acceptable only if file permissions and deployment boundaries are
  treated as the protection layer; stronger secret storage remains a follow-up.
- Full OIDC features such as signed ID tokens and JWKS may change the validation
  path later. The stable dependency for now is token exchange plus `userinfo`.
- If multiple Slock servers are connected, UI and tools need clear linker
  selection to avoid reading from the wrong workspace.

## Source Journals

- None yet. This is the initial canonical specification for Slock OAuth linkers.
