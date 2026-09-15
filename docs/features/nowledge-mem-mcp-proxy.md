# Nowledge Mem MCP Proxy

Status: integration in progress. Scoped proxy startup and the shared operation journal exist;
context bootstrap and authority-alias reconciliation remain open. This is the initial Mem seam for the
[loop product model](agent-loop-product-model.md).

## Problem

Team agents need to use an existing Nowledge Mem deployment without making
Nowledge Mem responsible for AgentHub task ownership, ACP session recovery, or
credential delivery. The existing AgentHub ACP bootstrap reads a static local
MCP configuration, which cannot bind a Team run to an existing Mem space or
preserve the write-recovery boundary required by Mem's current MCP contracts.

The current implementation resolves existing profiles into the shared daemon proxy and supplies
local ACP stdio descriptors after verifying upstream namespace authorization. Context Lens bootstrap
remains integration work; the full contract below is not yet a completion claim.

## Scope

- Local ACP Team members only.
- Existing Nowledge Mem `POST /mcp` transport and existing tool contracts.
- Team/project-to-space binding and optional actor-to-credential-profile
  binding.
- Runtime schema discovery, Context Lens bootstrap, and local operation audit.

## Non-Goals

- Changing Nowledge Mem APIs, schemas, storage, authorization, or task model.
- Mirroring AgentHub tasks, mailbox state, ACP transcripts, or provider
  sessions into Nowledge Mem.
- Creating Nowledge Mem actors, workspaces, spaces, or credentials.
- Remote-node support until AgentHub has an existing secret broker.

## Architecture

AgentHub starts a local stdio MCP proxy for an explicitly bound local Team
member. The provider ACP process only sees that stdio server. The proxy owns
the upstream connection to the already configured Mem `POST /mcp` endpoint.

The proxy is intentionally transparent:

- It initializes upstream and uses `tools/list` as the schema for that run.
- It exposes the upstream tool names, schemas, and results unchanged.
- It injects or validates `space_id` only when that exact tool schema declares
  the field. It must not add fields to schemas or calls that do not allow them.
- It preserves MCP `isError` responses, JSON-RPC failures, and successful
  envelopes containing an error value. AgentHub may classify these locally for
  audit, but it does not replace them with an invented error contract.

### Bindings

`MemScopeBinding` is owned by AgentHub and maps a Team/project scope to an
existing endpoint/profile reference and an existing `space_id`.

`MemActorBinding` is optional and maps a stable AgentHub actor to an existing
credential profile reference. A temporary worker may instead use the
supervising member profile. Bindings store references only; bearer keys and
refresh material remain in the configured secret source.

Scope and credentials are deliberately independent. A credential must never
select a space implicitly for an AgentHub Team run.

The resolver uses the existing `nowledge_mem.profiles` and `team_bindings` configuration, including
actor profile overrides. It resolves `credential_env` in the daemon and sends a Bearer header to
the configured HTTPS endpoint (loopback HTTP is allowed for local servers). URLs containing user
information, a query, or a fragment are rejected. Optional `tool_set` becomes `X-Nmem-Tool-Set`;
it does not grant authorization or retry permission.

The launch fingerprint includes configuration references and the bound space, but excludes the
resolved key and activation credential-file path. ACP receives only a local shim command and
that file reference. Provider launch strips credentials for every configured Mem profile as well
as ambient Mem/header variables before spawning the process. Remote loop execution remains denied.

For schemas declaring `space_id`, a missing value is injected and any supplied value other than
the exact bound string is rejected, including null and non-string values. Schemas without that
property remain unchanged; the verified upstream credential enforces their namespace. Schema
preservation and tool-set filtering alone do not establish that authorization.

### Upstream authorization

The configured Mem deployment must expose the existing authenticated `GET /members/me` contract
beside its `/mcp` route. The daemon sends the exact credential retained for MCP and requires:

- a valid workspace UUID;
- `key_scope.scope_mode` equal to `narrowed`, with exactly one grant equal to the explicit Team space;
- a matching mint-time write space, or its documented null personal-space default;
- `key_write_target.write_space` equal to the bound space and `write_space_live` equal to true.

The scope declaration never comes from provider JSON. A full key, extra grants, a foreign target,
revoked/absent authorization, an inactive destination, or a malformed response prevents mounting and
provider startup. The authenticated workspace identity contributes to the launch fingerprint without
persisting the membership response or credential value. Configuration-only preflight remains offline.

The probe preserves the endpoint's path prefix, has a ten-second deadline and a 64 KiB response
limit, and follows neither redirects nor ambient proxy configuration. Failures omit the URL, response
body, and credential. No schema or authorization changes are required in Mem.

Mem's narrowed-key authorization is immutable at mint apart from revocation; changing its reach
requires another credential. Every subsequent MCP request uses that verified credential, and Mem
applies its current member/key grants at request admission. Tools without a scope field and opaque
resource/object identifiers therefore retain their native wire contract under upstream enforcement.
Standard resources, templates, prompts, completion, logging, and callbacks are available subject to
upstream support and the shared proxy's method/capability checks. Upstream errors remain intact.

This contract is implemented by Mem Cloud. A desktop/Family endpoint without narrowed-key authority
fails explicitly. A space header, tool-set selection, or exact-space protocol acknowledgment cannot
replace authorization, and AgentHub does not create a space or mint a credential to bypass the gate.

### Context Lens

At loop activation the proxy calls `read_context_bundle` for the bound scope. The
returned content, including its contract line that identifies it as attributed
data rather than instructions, is passed through unchanged into the AgentHub
runtime context. This is a read-only scope lens; it is not provider-session
resume state.

Stable actor and Team/project bindings survive process exit. Activation/session IDs provide
correlation, not new Mem spaces or user identities. Canonical tasks and IM supply current work;
agents retrieve relevant prior knowledge through the discovered Mem tools.

### Operation Journal

The shared [MCP operation journal](mcp-operation-journal.md) owns persistence and replay
enforcement. Mem uses the same status/error types and store as future app integrations.

The AgentHub-local journal records only endpoint/profile references, tool name,
scope, correlation id, status, Mem pointer, and a redacted safe summary. It
does not store credentials, memory bodies, thread bodies, or raw diagnostic
envelopes.

The state transition is:

```text
prepared -> sent -> succeeded
                 -> failed
                 -> outcome_unknown
```

Reads and writes known not to have been sent may be retried. A non-idempotent
write whose outcome becomes unknown after `sent` must remain
`outcome_unknown`; it must not be replayed. A retry is allowed only when the
actual schema advertises stable caller identity and the original call used the
same stable value.

## Contracts

### Local-Only Security

- The provider ACP payload, Team spec, normal environment dumps, and logs must
  not contain the upstream URL, bearer token, refresh material, or ambient Mem
  headers.
- The local proxy resolves credentials through an existing secure profile or
  environment reference and never persists the resolved secret.
- Starting the integration on a remote node fails closed until an existing
  secret broker can deliver the referenced credential there.

## Delivery Plan

1. Add binding validation and a local-only capability gate.
2. Add the stdio MCP proxy with upstream initialization and dynamic tool
   discovery.
3. Add Context Lens bootstrap and the redacted operation journal.
4. Add focused protocol, scope, ambiguous-write, and remote fail-closed tests.

## Validation Matrix

- A bound local Team member receives only tools returned by the upstream
  `tools/list` call.
- A tool with declared `space_id` receives the bound space; a tool without it
  is forwarded without an extra property.
- Full/multiple/foreign/empty grants and inactive write destinations fail before provider startup;
  bounded HTTP membership checks keep credentials on the configured origin. A configured ACP
  provider exercises native scoped/unscoped tool schemas, resource reads, prompt retrieval, and
  preserved upstream denials through the actual shim and signed RPC path.
- Context Lens content and its contract line reach the runtime unchanged.
- MCP result errors, JSON-RPC errors, and envelope errors are each preserved
  and journaled without bodies.
- A disconnected non-idempotent write becomes `outcome_unknown` and is not
  replayed.
- A remote member with a Mem binding fails before provider startup when no
  secret broker is configured.

## Operational Notes

Local loop outcomes and Mem writes are separate. Missing knowledge needed for an action creates a
visible wait; independent work may continue without claiming successful retrieval. Local Team
delivery is the first slice; standalone/remote coverage needs explicit scope and credential designs.

## Open Risks

- The configured deployment must implement the narrowed-key contract; schema/route availability
  alone does not prove its authorization behavior. Deterministic fixtures do not validate a live
  Mem deployment's configuration.
- Endpoint aliases still need explicit reconciliation of existing journal scope identities.
- Ambiguous non-idempotent writes require reconciliation across future activations.
- Filesystem knowledge needs selective migration with provenance, not an automatic workspace upload.

## Source Journals

- [Shared MCP proxy checkpoint](../journal/2026-09-15-shared-mcp-proxy.md)
- [Loop product definition](../journal/2026-09-15-agent-loop-product-definition.md)
