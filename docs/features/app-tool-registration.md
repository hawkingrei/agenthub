# App Tool Registration

Status: target integration design for the [loop product model](agent-loop-product-model.md).
The [registry storage foundation](app-registry-storage.md) is implemented; public management APIs,
runtime integration, and event ingress remain pending.

## Problem

Agent tool surfaces are currently built in: IM, task list, execution, scheduling/follow-up, loop
completion, and the Nowledge Mem proxy. Extending an agent with a new capability requires changing
AgentHub itself. External applications need a registration interface that lets them declare
callable tools and deliver events to agents directly — without becoming part of the runtime and
without weakening task, IM, scheduling, or authority boundaries.

## Scope

- App registration identity, manifest-declared tools, and manifest versioning.
- Binding registered tools into Team/agent tool sets and loop activations.
- Invocation identity, credential handling, redaction, and fail-closed behavior.
- App-emitted events as durable activation triggers.
- Observability of registered-tool calls and app events in the activation trace.

## Non-Goals

- A public marketplace, app review pipeline, or billing.
- In-process plugins or arbitrary code execution inside AgentHub.
- Replacing provider-internal tools or the canonical task/IM authorities.
- Remote command execution of apps against agents; events are notifications, not commands.
- Concrete registry APIs, wire schemas, or storage migrations in this document.

## Architecture

### Registration Objects

| Object | Responsibility | Lifetime |
| --- | --- | --- |
| App registration | Stable app identity, owner, endpoints, and credential reference | Until revoked |
| Tool manifest | Versioned declaration of tools: name, input/output JSON Schema, required scopes, idempotency | Superseded by new versions |
| Binding | Operator grant of one app to a Team/agent with an approved scope subset and version policy | Until unbound or revoked |
| Tool call | One invocation attributed to actor, Team scope, and activation | Execution episode |
| App event | Signed app-to-AgentHub notification that may become an activation trigger | Durable until consumed |

This follows the established app-platform pattern: an app registers once with a stable identity
and a manifest, the platform discovers callable operations from the manifest, agents invoke them
under their own asserted identity, and app-to-platform events flow through a separate signed
channel instead of remote control of agents.

### Invocation Path

MCP is the canonical invocation protocol. A registered app exposes an MCP server (local command or
remote HTTP endpoint); a plain HTTP action service can be adapted behind the same seam. Agents call
registered tools through a local enforcement proxy, generalizing the existing
[Mem MCP proxy](nowledge-mem-mcp-proxy.md) design:

```text
Agent (loop activation)
  -> local tool proxy (identity injection, scope filter, redaction, operation journal)
  -> registered app endpoint (MCP server or adapted HTTP action service)
```

The proxy discovers upstream tools, intersects them with the manifest and the binding's approved
scopes, and republishes only approved tools with their upstream schemas and results. Tools the
manifest does not declare never reach an agent, even when the upstream server offers them.

## Contracts

### 1. Registration And Manifest

- An app registers a stable app id, an owner, endpoints, and a credential reference. Secrets stay
  in server-side secret storage; registration never returns them to prompts, agent context, Agent
  Cards, or logs.
- The manifest is versioned and declares every tool: stable name, input/output JSON Schema,
  required scopes, and whether the tool is idempotent. Undeclared tools and undeclared scopes are
  rejected at call time, not only at review time.
- A manifest update creates a new version; bindings pin or track versions explicitly. A schema
  change must not silently change a bound tool inside an active loop: the activation's recorded
  configuration reference includes the bound manifest versions.
- Growing an app's operation surface indefinitely through manifest actions is an anti-pattern.
  When the surface approaches a second SDK, prefer an authenticated CLI or service boundary and
  keep the manifest small.

### 2. Binding And Authority

- An operator binds an app to a Team or agent with an approved scope subset. Binding is
  configuration, not code; the Agent Card lists bound tool capabilities without credentials.
- The launch resolver resolves bound registered tools into the activation's tool set alongside the
  built-in surfaces in the [runtime tool table](agent-loop-runtime.md#5-tool-responsibilities).
  Existing role authorization still applies: a registered tool cannot complete tasks, deliver IM,
  schedule activations, or extend authority beyond its approved scopes.
- Canonical state stays canonical. Apps receive task/thread references as opaque identifiers and
  must not become a second task ledger, inbox, or scheduler, mirroring the Mem boundary.
- Unbinding or revoking an app invalidates future calls immediately and revokes issued tokens.
  In-flight activations receive explicit tool errors, not a silently shrunk tool set.

### 3. Invocation Identity And Redaction

- Every call carries the stable actor identity, Team/workspace scope, and the current activation
  id; provider session identifiers stay internal. AgentHub asserts agent identity to the app and
  authenticates with the app credential; identity never depends on prompt text.
- The proxy journals tool name, target reference, status, duration, and operation identity —
  never raw payload bodies in diagnostics. A non-idempotent call with an unknown outcome follows
  the same ambiguous-write rule as Mem: recorded, reconciled, and not blindly replayed by the next
  loop.
- Fail closed. An unavailable or misbehaving app yields a visible bounded tool error. Work that
  requires the tool records a wait; independent work continues without claiming the call
  succeeded.

### 4. App Events As Triggers

- A registered app may deliver signed events to AgentHub. Events enter the same durable activation
  intake as user and agent triggers under
  [agent-initiated scheduling](agent-loop-runtime.md#7-agent-initiated-scheduling); they never
  execute anything directly.
- A binding declares which event classes may activate which members; an undeclared event class is
  dropped with a journal record. Standing triggers may reference declared app event conditions.
- Event delivery requires verifiable app identity and replay protection: per-app event id plus a
  monotonic cursor. Duplicate events coalesce, and event storms are bounded by the same per-actor
  and per-team budgets as every other trigger source.

### 5. Observability

- Registered-tool calls appear in the activation trace as tool-boundary summaries tagged with app
  id and manifest version; app events appear as trigger sources with their event identity, per the
  [observability contract](agent-loop-runtime.md#8-observability-and-activation-trace).
- `agenthub doctor agent-trace` attributes a loop stalled on an app call to the app boundary,
  distinct from the provider, permission, persistence, and SSE layers.

## Validation Matrix

| Boundary | Required future check |
| --- | --- |
| Registration | Undeclared tool/scope rejected at call time; secrets absent from context, Cards, and logs |
| Versioning | Bound manifest version recorded per activation; a schema change never mutates an active loop |
| Binding | Unbind/revoke invalidates future calls and tokens; in-flight calls fail explicitly |
| Identity | Calls carry actor and activation identity without provider-session leakage |
| Ambiguity | Unknown-outcome non-idempotent call is recorded and never auto-replayed |
| Fail-closed | Unavailable app produces a bounded visible error and a recorded wait |
| Events | Signature and replay checks enforced; undeclared classes dropped; storms hit trigger budgets |
| Trace | App calls and events attributed in the activation trace and doctor output |

## Operational Notes

The first slice reuses the Mem proxy machinery — scope binding, error classification, and the
operation journal — for one registered MCP app bound to a local Team member. HTTP action
adaptation, app events, and remote-member credential delivery follow as separate slices tracked in
[TODO](../todo.md#agent-loop-product-transition). Keep one proxy/policy implementation shared with
the Mem integration rather than a parallel enforcement stack.

## Open Risks

- Schema-only review cannot prove app behavior; fail-closed defaults and narrow scope bindings
  limit the blast radius of a misbehaving app.
- Two extension seams (Mem proxy and the app registry) can drift apart; sharing the proxy and
  policy implementation is a requirement, not an optimization.
- Event-driven activation expands the trigger surface; budgets, signatures, and attribution must
  land with the first event slice, not after it.
- Remote members need the same credential-delivery design as the Mem integration before parity
  claims.

## Source Journals

- [Loop product definition](../journal/2026-09-15-agent-loop-product-definition.md)
