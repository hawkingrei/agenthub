# App Linkers Specification

## Problem

AgentHub needs a provider-neutral way to connect external applications so
agents can inspect and act on external context without receiving raw third-party
tokens, learning provider-specific login rituals, or crawling arbitrary browser
routes.

Slock is only one provider. The stable product abstraction is an App Linker:

```text
external app connector -> configured linker -> linked principal -> bounded tools
```

The same abstraction should support OAuth-like apps, API-key apps, local bridge
apps, and future plugin-wrapped connectors while preserving one rule: AgentHub
owns the connection boundary, authorization, auditing, and response shaping.

## Scope

- Define the generic App Linker domain model.
- Separate provider adapters from configured connection instances.
- Support server-side credential storage and redacted browser responses.
- Provide a root-owned Admin surface for configuring and linking apps.
- Define the canonical agent access path for external app tools.
- Support provider-specific read tools with bounded response shapes.
- Leave room for plugin packaging and event subscriptions without making either
  the only integration mechanism.
- Keep provider credentials, access tokens, callback codes, and raw app secrets
  out of agent prompts, browser JavaScript, logs, screenshots, and tool output.

## Non-Goals

- Replacing AgentHub local authentication with third-party login.
- Making every external app use OAuth.
- Building a generic unrestricted HTTP proxy.
- Letting agents call provider internal APIs directly.
- Exposing raw access tokens or provider secrets through actor tools.
- Syncing all external app history into AgentHub storage by default.
- Defining provider-specific endpoint paths in this generic spec.
- Treating plugin packaging as a substitute for AgentHub-side authorization.

## Architecture

### 1) Domain Model

`AppConnectorId` identifies a provider adapter and its stable capability
surface. It is not a specific login, token, tenant, user, or workspace.

Examples:

```text
slock
github
linear
notion
```

`AppLinkerId` identifies one configured connection instance inside AgentHub.
It is the handle admins and tools use to address a concrete external app
connection.

Examples:

```text
slock-primary
github-hawkingrei
linear-agenthub
notion-product-docs
```

`LinkedPrincipal` identifies the external subject that authorized the linker.
The principal may be a human, an agent, a service account, or a provider-specific
bot, depending on the connector.

`AppResource` identifies a provider resource type that AgentHub exposes through
bounded tools. Examples include channel, message, issue, pull request, page,
task, file, event, or search result.

`AppSubscription` identifies an optional event stream or polling cursor that
imports provider updates into AgentHub. Subscriptions are not required for
simple request/response tools.

### 2) Connector Adapter

Each connector adapter owns provider-specific behavior:

- supported auth modes
- required config fields
- requested scopes or permissions
- token exchange and refresh behavior
- external subject mapping
- resource method implementations
- response shape normalization
- provider-specific rate-limit handling

Adapters must expose named methods instead of arbitrary URL forwarding.

Reference adapter surface:

```text
ConnectorAdapter.describe()
ConnectorAdapter.validate_config(config)
ConnectorAdapter.start_link_attempt(linker_id, user_id)
ConnectorAdapter.exchange_callback(link_attempt, callback)
ConnectorAdapter.refresh_if_needed(linker_id)
ConnectorAdapter.list_resources(linker_id, resource_type, query)
ConnectorAdapter.invoke_tool(linker_id, tool_name, input)
ConnectorAdapter.sync_subscription(linker_id, subscription_id)
```

### 3) Linker Service

The Linker Service is the root of trust for all app connections.

It owns:

- database schema
- credential redaction
- link attempt state
- token exchange and refresh orchestration
- linked principal snapshots
- internal authorization checks
- audit emission
- tool response bounds

The canonical runtime path is:

```text
agent -> AgentHub tool/CLI/MCP wrapper -> internal authorization -> linker service -> connector adapter -> provider API
```

Agents may use tools exposed by AgentHub. They must not receive raw provider
credentials or call provider APIs through an unrestricted proxy.

### 4) Plugin Wrapping

An App Connector can be packaged as a plugin, but the plugin does not own the
trust boundary by itself.

Valid plugin responsibilities:

- registering connector metadata
- adding UI affordances for provider-specific fields
- contributing tool descriptions and schemas
- implementing provider adapter methods when loaded server-side
- exposing subscription processors

Invalid plugin responsibilities:

- bypassing AgentHub redaction
- returning access tokens to agents
- requiring a separate agent-only login protocol
- proxying arbitrary provider URLs
- storing provider secrets in browser-only state

### 5) Subscriptions And Messages

Some apps provide message-like surfaces such as channels, inboxes, comments, or
events. AgentHub models them as resources first, not as native Team messages by
default.

Provider messages can enter AgentHub in two ways:

- direct query: an agent calls a bounded read tool.
- subscription: AgentHub polls or receives provider events and stores normalized
  references or summaries.

Subscriptions must define:

- resource type
- cursor or checkpoint
- owner linker
- required scopes
- retention and payload bounds
- failure backoff
- audit behavior

External messages are not automatically Team conversation messages. A connector
must explicitly map an external event into a Team channel, task note, or agent
mailbox item.

## Contracts

### 1) Connector Descriptor

Each connector exposes stable metadata:

```json
{
  "connector_id": "slock",
  "display_name": "Slock",
  "auth_modes": ["oauth_code"],
  "resource_types": ["channel", "message"],
  "tool_names": ["slock.channels.list", "slock.messages.list"],
  "supports_subscriptions": false
}
```

`connector_id` is stable and lower-case. It should not include tenant, account,
or environment identifiers.

### 2) Linker Record

Admin APIs return redacted linker records:

```json
{
  "linker_id": "slock-primary",
  "connector_id": "slock",
  "display_name": "Slock",
  "status": "connected",
  "client_id": "agenthub",
  "credential_configured": true,
  "token_configured": true,
  "expires_at": 1779168000,
  "principal": {
    "subject": "27a3edb7-4e03-4a42-a61d-63fc04fce62c",
    "principal_type": "agent",
    "display_name": "Claude Assistant",
    "handle": "assistant"
  }
}
```

Records must not include raw credentials, access tokens, refresh tokens,
callback codes, provider session cookies, or raw provider payloads that contain
secrets.

### 3) Storage Model

The generic storage model separates public config, secrets, principals, and
link attempts:

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
- credential fields
- access token fields
- expiry fields
- updated_at

app_linker_principals
- linker_id
- subject
- principal_type
- display_name
- handle
- avatar_url
- provider workspace fields
- raw identity snapshot
- updated_at

app_linker_attempts
- state
- linker_id
- created_by_user_id
- expires_at
- created_at
```

Provider adapters may extend `config_json` and identity snapshots, but must not
change the redaction rule.

### 4) Admin API

Generic route shape:

```text
GET    /api/admin/linkers
GET    /api/admin/linkers/:linker_id
PUT    /api/admin/linkers/:connector_id
POST   /api/admin/linkers/:connector_id/link_attempts
POST   /api/admin/linkers/:connector_id/exchange
DELETE /api/admin/linkers/:linker_id
```

Provider-specific routes may exist when the provider needs a stable callback or
resource path:

```text
GET /api/linkers/:connector_id/callback?code=...&state=...
GET /api/admin/linkers/:connector_id/resources/:resource_type
```

All admin routes require `root`. Public callback routes rely on server-side
link attempt state, not on browser local storage or bearer tokens.

### 5) Tool Contract

Agent-facing tools use stable connector tool names:

```text
app.linkers.list
app.resources.list --linker-id <id> --resource-type <type>
app.tool.invoke --linker-id <id> --tool-name <name> --input <json>
```

Provider-friendly aliases are allowed:

```text
slock.channels.list --linker-id slock-primary
github.pull_requests.list --linker-id github-hawkingrei
notion.pages.search --linker-id notion-product-docs
```

Tool responses must be bounded, typed, and redacted. Large provider payloads
should return summaries, cursors, or artifact pointers instead of unbounded
inline data.

### 6) Authorization Contract

The first generic permissions are:

```text
app_linker:read
app_linker:invoke_read_tool
app_linker:invoke_write_tool
app_linker:admin
```

`app_linker:admin` is root-only in the first version.

Read tools and write tools must remain distinct. A connector that only supports
read tools must not accidentally inherit write permissions through a generic
invoke path.

### 7) Audit Contract

AgentHub records audit events for:

- linker config created, updated, or deleted
- link attempt created, completed, expired, or failed
- linked principal changed
- token refresh succeeded or failed
- agent tool invoked a provider read method
- agent tool invoked a provider write method
- subscription sync completed or failed

Audit records must not include credentials, tokens, callback codes, provider
session cookies, or full unbounded external payloads.

## Validation Matrix

| Area | Validation |
|---|---|
| Connector identity | `connector_id` remains provider-level and does not encode account or tenant state. |
| Linker identity | Multiple linkers can share one connector without overwriting each other. |
| Secret redaction | Browser and agent responses never include raw credential or token fields. |
| Root-only admin | Non-root users cannot create, update, exchange, or delete linkers. |
| Callback state | Public callbacks require a live single-use state bound to a linker attempt. |
| Principal mapping | Provider identity snapshots map into stable `subject` and `principal_type` fields. |
| Tool authorization | Agent tool calls require explicit app-linker permissions. |
| Resource bounds | Resource reads clamp page size and return cursors or bounded result sets. |
| Plugin loading | Plugin-provided adapters cannot bypass server-side redaction and authorization. |
| Subscription sync | Subscription processors persist cursors and fail closed on auth or schema errors. |

Focused test targets:

```text
cargo test -p agenthub app_linker
cargo test -p agenthub slock
cd web && npm run test -- src/use_app_admin.test.tsx src/pages/admin_page.test.tsx src/pages/admin_page_sections.test.tsx
cd web && npm exec tsc -- --noEmit
```

## Operational Notes

- The Admin UI should show configured, connected, expired, and failed states
  without exposing secret values.
- Link attempts should be short-lived and single-use.
- Expired or disconnected linkers must fail closed for agent tools.
- Provider rate limits should surface as controlled tool errors with retry
  metadata when available.
- Secret storage starts with SQLite-backed server-side fields. Stronger envelope
  encryption or OS keychain integration can be added without changing the public
  linker contract.
- Provider resource APIs should use small default page sizes and explicit
  cursors.

## Open Risks

- Provider-specific resource endpoint contracts may lag behind login contracts.
  Generic linkers must not turn guessed provider URLs into public API promises.
- Multi-linker UI needs clear selection semantics before many accounts of the
  same provider are common.
- Write-capable tools need stricter human approval, audit, and rollback design
  than read tools.
- Plugin packaging needs a stable server-side adapter loading model before
  third-party connector plugins can be trusted in production.
- Subscriptions need retention and deduplication rules before external messages
  can be imported into Team conversation surfaces.

## Source Journals

- None yet. This is the initial canonical specification for the generic App
  Linker abstraction.
