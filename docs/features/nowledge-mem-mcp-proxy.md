# Nowledge Mem MCP Proxy

## Problem

Team agents need to use an existing Nowledge Mem deployment without making
Nowledge Mem responsible for AgentHub task ownership, ACP session recovery, or
credential delivery. The existing AgentHub ACP bootstrap reads a static local
MCP configuration, which cannot bind a Team run to an existing Mem space or
preserve the write-recovery boundary required by Mem's current MCP contracts.

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

### Context Lens

At run start the proxy calls `read_context_bundle` for the bound scope. The
returned content, including its contract line that identifies it as attributed
data rather than instructions, is passed through unchanged into the AgentHub
runtime context. This is a read-only scope lens; it is not provider-session
resume state.

### Operation Journal

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

## Local-Only Security Contract

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
- Context Lens content and its contract line reach the runtime unchanged.
- MCP result errors, JSON-RPC errors, and envelope errors are each preserved
  and journaled without bodies.
- A disconnected non-idempotent write becomes `outcome_unknown` and is not
  replayed.
- A remote member with a Mem binding fails before provider startup when no
  secret broker is configured.
