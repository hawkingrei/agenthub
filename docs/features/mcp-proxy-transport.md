# MCP Proxy Transport

## Problem

Loop MCP integrations need transparent protocol transport while keeping upstream configuration and
credentials in the daemon. A transport that retries a write automatically can bypass the durable
operation journal. Buffering complete SSE responses would also prevent progress and server request
handling while a tool is running.

## Scope

`agenthub-mcp` supplies JSONL framing, JSON-RPC envelope inspection, protocol lifecycle state,
Streamable HTTP request preparation, and incremental JSON/SSE response handling. The implementation
is tested with local fake upstreams. The same crate now provides trusted discovery/call policy and
actual HTTP/journal orchestration. Signed streaming RPCs and a local stdio shim exercise the
provider-facing bridge. Existing Mem profiles now resolve to activation mounts and local ACP
descriptors with provider environment isolation. The remaining protocol controller and complete
integration authorization still gate completion of slice 9.

## Non-Goals

- Granting actor, Team, task, or binding authority through protocol metadata.
- Reconstructing an upstream result from an operation receipt.
- Treating an HTTP acknowledgment, MRTR input request, or task acceptance as business completion.
- Automatically resending a tool call, parsing opaque request state, or inventing server capabilities.

## Architecture

The daemon constructs `McpHttpTransport` from a trusted endpoint and credential headers. Request
preparation performs no I/O and returns an owned, non-cloneable request. The caller must commit its
[journal send boundary](mcp-operation-journal.md) before consuming that request in `send`.

The HTTP exchange exposes session metadata separately from protocol messages. `next_event` yields
messages incrementally together with optional upstream cursor/retry control information. Only MCP
messages belong in the provider's JSONL stream. Session material, credentials, and request URLs do
not have a diagnostic or serialization implementation.

Raw transport modules do not access the database. The crate's policy and journal modules combine
the shared domain/store with the transport: scope-bound request preparation fixes immutable wire
intent, and a journaled client consumes the prepared request only after obtaining a send permit.
The daemon session bridge connects signed activation RPCs to this path. A local `agenthub mcp-proxy`
process translates provider JSONL into those RPCs; upstream configuration stays in daemon memory.
Explicit Team Mem bindings are mounted after the immutable launch snapshot is recorded, while the
activation operation guard prevents cleanup from racing publication. Unbound activations mount no
Mem server. Fresh and resumed local ACP sessions receive the same typed stdio descriptor path.

## Contracts

### Configured launch

- Reuse the existing Team/actor profile resolver. The actor may select a credential profile but
  cannot change the Team's `space_id`. Endpoint and credential references come from daemon config.
- Resolve the referenced secret inside the daemon. Accept HTTPS, or HTTP on a loopback host, with
  no URL user information, query, or fragment. Use the configured `/mcp` endpoint and tool-set header;
  a tool set is discovery configuration, not authorization or permission to replay a write.
- Fingerprint endpoint/profile/credential references, space, tool set, and access-policy version. Secret values and the
  per-activation credential-file path are excluded, so rotation does not change configuration identity.
- ACP receives only the local `agenthub mcp-proxy --server-id nowledge-mem` command and its
  activation credential-file reference. No descriptor field carries the upstream URL or headers.
- Before spawning a loop provider, remove all configured Mem credential variables and ambient
  `NMEM_*`, `NOWLEDGE_MEM_*`, and MCP header variables, including provider-specific overrides.
  Descendant shims inherit that sanitized environment. The legacy static MCP loader is unchanged.
- All discovered Mem tools currently use the conservative non-idempotent journal policy. Read and
  stable-identity retry declarations, Context Lens bootstrap, and complete scope authorization
  belong to the subsequent Mem integration; transport availability alone does not prove them.
- The configured Mem binding currently grants tool access and standard tool callbacks. It denies
  resource, prompt, completion, logging-control, and resource-subscription requests locally.
  These surfaces need independent upstream namespace authorization before they can be enabled;
  a routing header or an unscoped schema is insufficient. This boundary does not complete Mem
  scope integration, including tools that omit a declared `space_id`.

### Legacy static MCP compatibility

Local ACP launches without a loop launch configuration continue loading the existing static MCP
configuration for both fresh sessions and session loading. Native stdio command, arguments, and
environment entries pass to the provider unchanged. HTTP URLs and headers remain native descriptors
and are included only when the provider advertises HTTP MCP support. Loop launches use their
resolved proxy descriptors and never invoke the ambient static loader.

### Integration access policy

Every binding supplies a trusted `McpAccessPolicy` independently of upstream discovery. Its default
is no data-surface access. Tool names, resource URIs, resource template identifiers, prompt names,
and callback methods have separate grants: none, exact named entries, or the entire surface when
the integration has independently established authority. A resource template is not a prefix or
permission to read every expanded URI. These grants cannot be supplied by provider JSON.

Single requests, every March batch member, and modern subscription filters pass the same policy
before committing request IDs or lifecycle changes. Task queries and task subscriptions additionally
retain their existing receipt, current catalog, and executor checks. Callback IDs acquire response
authority only after their incoming method passes policy. Modern deferred `inputRequests` use those
same callback grants, including task creation/query responses and task notifications. Opaque tool
result data and JSON-RPC error details are not interpreted as input requests. A revoked prepared exchange cannot start;
previously admitted sends still finish their factual journal drain.

Discovery hides unapproved tools, resources, templates, and prompts while preserving the schema,
arguments, extensions, ordering, and pagination of retained entries. Initialization and modern server
discovery omit capabilities for disabled surfaces, including the modern tasks extension when tools
are disabled. Modern logging opt-in and server log notifications also require the logging grant.
A returned resource read containing an unapproved
URI is rejected before provider delivery, including when a deferred result carries resource content.
Errors retain upstream JSON-RPC data. Valid deferred read envelopes remain unchanged and acquire
the session-local continuation receipt described below. Discovery cannot return a deferred result.

Named grants constrain protocol access; they do not prove upstream credential or namespace isolation.
Integration configuration must bind arguments and establish that the server enforces the intended
namespace, especially for opaque object IDs and prompts whose arguments select data.

### Client capability negotiation

Client support is independent of integration permission. Legacy sessions retain a fixed-size
projection of the initialization declaration and clear it after a failed handshake. Modern exchanges
snapshot their own `io.modelcontextprotocol/clientCapabilities` metadata; concurrent requests,
task queries, and subscriptions never borrow another request's declaration. Raw capability metadata
continues upstream unchanged, without retaining arbitrary capability objects in proxy state.

Direct callbacks and deferred input requests require the same declared roots, sampling, or elicitation
capability. This includes tool/read results, task creation/query results, and task notifications.
Sampling `tools` and `toolChoice` require `sampling.tools`. Elicitation mode defaults to form;
an empty elicitation declaration supports form only, and explicit form/URL modes require their
corresponding declaration. Unknown extension methods remain subject to integration grants without
inventing capability semantics. Opaque content and error data are not parsed as deferred inputs.

Modern `notifications/message` requires the originating request's `io.modelcontextprotocol/logLevel`
and cannot fall below that severity. Missing opt-in or an invalid emitted level rejects delivery;
invalid requested levels fail admission. Legacy logging retains its server-controlled level behavior.
Capability rejection occurs before registering any callback IDs in a frame. Already admitted writes
and task observations still complete their factual journal work before rejected delivery closes.

### Version and lifecycle

| Version | Transport behavior |
| --- | --- |
| 2025-03-26 | Initialize handshake, optional HTTP session, GET stream/resumption, JSON-RPC batches |
| 2025-06-18 and 2025-11-25 | Initialize handshake, optional HTTP session, GET stream/resumption, single-message envelopes |
| 2026-07-28 | Per-request version/client metadata, standard/custom request headers, request-scoped SSE, subscriptions and MRTR results |

The legacy lifecycle adopts the upstream's supported version and unchanged capabilities. Tools
cannot start before initialization and the initialized notification. An upstream initialization
error remains an error; the session does not manufacture a successful handshake. Unknown versions
fail explicitly. Modern requests do not receive a synthetic legacy initialization response.

Modern requests cannot invoke retired standard methods: legacy initialization notifications, ping,
client progress/root-change notifications, logging level controls, resource subscribe/unsubscribe,
or `tasks/result`. Legacy requests cannot invoke modern discovery, subscriptions, or task updates.
Equivalent legacy methods remain available under their negotiated version and integration grants.

Failed handshakes retain lifecycle admission while admitted callback replies settle. Pending
callback IDs, discovery state, and provisional HTTP sessions are then retired before another
initialize can begin. Upstream errors remain unchanged; a new request ID may start a fresh
handshake after successful cleanup, including reusing callback IDs in the new upstream session.
Failed DELETE closes the proxy and retains the private context for final shutdown. A repeated
initialized notification during normal operation is forwarded as an ordinary notification; its
error does not reset the operating session or delete sessions with admitted tool calls. The
controller distinguishes the initial transition described by the
[legacy lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle)
from subsequent notifications.

Modern `server/discover` is forwarded with its per-request metadata and HTTP method/version
headers, without an HTTP session. Supported versions, capabilities, server metadata, instructions,
cache hints, and extensions remain upstream data. The proxy does not cache this response or treat
it as execution authority. An upstream probe error remains unchanged and leaves the legacy
initialize path available for a client-selected fallback. This follows the pinned
[discovery contract](https://modelcontextprotocol.io/specification/2026-07-28/server/discover).

The protocol boundary follows the primary [2025-03 transport](https://modelcontextprotocol.io/specification/2025-03-26/basic/transports),
[2025-11 transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports), and
[2026-07 transport](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http)
contracts. The modern [versioning contract](https://modelcontextprotocol.io/specification/2026-07-28/basic/versioning)
differs from legacy handshake negotiation; the adapter must preserve this distinction.

### HTTP and JSONL

- JSON-RPC methods, IDs, permitted schemas and result content, and upstream error data remain unchanged.
  Discovery applies the access projection above. JSON serialization can change whitespace or object ordering.
- Each message is one JSONL line. Reads reject unterminated or malformed frames and enforce an
  8 MiB message limit before growing a line buffer beyond the limit.
- HTTP POST accepts JSON or SSE. Legacy notifications/responses require an empty 202 acknowledgment.
  Valid JSON-RPC error bodies remain available even when carried by an HTTP error status.
- Automatic HTTP retries and redirects are disabled. Ambient proxy variables are not consulted by
  this client; upstream routing and credentials come from the trusted configuration.
- Header construction and size/type validation happen before a request can be journaled as sent.
- Tool/control requests have one deadline covering the initial POST and cursor-based recovery.
  EOF without a recoverable cursor does not establish a tool outcome. Independent GET listeners
  bound connection setup and remain cancellable while idle.
- Legacy stream resumption constructs GET with the exact cursor. It never reconstructs a tool POST.
  The controller retains cursor and retry state for that exchange, never across unrelated requests.
- The SSE decoder accepts LF/CRLF/CR, a UTF-8 BOM, comments, multiline data, empty priming events,
  cursor updates, and retry hints. It does not dispatch an unterminated event on EOF.
- Legacy SSE can carry server requests and notifications. Modern SSE rejects independent server
  requests; their modern representation is an MRTR result.

### March batches

The March protocol carries each admitted array in one HTTP POST. Requests and notifications may
share an array; callback responses use a response-only array. Initialization itself is never
batched. Later negotiated versions reject arrays.

Validate every member before changing lifecycle, consuming callback IDs, or creating tool sends.
Each tool uses the existing catalog, scope binder, and replay policy. Discovery inside a batch
does not authorize another member. Members consume their individual tool/control slots while
sharing one bounded message workspace. Registered callback batches retain the independent
callback allowance and bypass initialization delivery's gate.

Commit all tool send transitions atomically, then correlate factual receipts by response ID.
Preserve upstream response order and individual/array shapes. Before forwarding a frame, persist
its matching tool receipts. If the stream ends after partial results, deliver queued known results
and close without a completion marker; only unresolved members become unknown. No member is
automatically resent. Discovery refreshes and prior-cursor pages retain distinct generations.

### Legacy stream recovery and closure

A legacy SSE response with a nonempty event cursor can resume through at most three GET attempts
within the originating request's deadline. Each GET uses that stream's exact cursor, original
session, endpoint, and credentials. Server retry hints remain minimum delays; if the deadline
cannot accommodate one, no early reconnect is sent. An empty cursor clears recovery permission.
There is no repeated tool POST, new operation, or replacement send permit. Partial batch receipts
remain durable while the original pending members await their resumed results.

After initialized delivery, a daemon readiness field on internal RPC frames enables the shim's
independent listener. Preparation alone cannot set readiness. This field never enters MCP stdout. One listener per session forwards server
requests and notifications and resumes its own cursor. HTTP 405 completes this optional listener
without failing the provider. Cursorless stream loss closes delivery rather than claiming recovered
continuity. Modern request-scoped streams do not use legacy GET recovery.

Listener admission uses signed bootstrap authority. Connection setup and each delivered message
hold a short executor guard; idle reads do not. Current owner, lease, membership, mailbox, and
activation phase are checked before delivery and once per second while idle. Session/binding
closure interrupts listening. Its independent workspace prevents idle listeners from consuming
tool or callback capacity.

Closure denies new calls immediately, drains admitted exchanges, then sends one bounded DELETE
with the private upstream session header. A successful response, 404, or 405 completes termination.
Caller loss and activation cleanup also perform this cleanup; no database transaction spans it.
An upstream session 404 preserves any real error and closes the old provider stream. Reopening
requires initialization of a new session, while the original write's journal receipt still prevents
unauthorized replay. Cursors remain transient; daemon restart uses journal ambiguity recovery.

### Modern metadata

Per-request protocol version and client capabilities are required. Client information is optional;
when supplied, it must contain string `name` and `version` fields. Capabilities are never inferred
from an earlier request.

`MCP-Protocol-Version` must match the request metadata. Standard method/name headers and discovered
`x-mcp-header` parameter mappings are generated without changing tool arguments. Nested mappings
must be statically reachable through `properties`. Invalid paths, duplicate header names, forbidden
types, and unsafe names invalidate a tool's header plan. Discovery must exclude such tools when
using HTTP, as required by the protocol.

Header values use the specified Base64 sentinel for non-ASCII/control characters, surrounding
whitespace, or a literal sentinel-shaped value. Primitive string/boolean/integer values retain
their meaning. Integer headers must fit the JavaScript safe range. Null or absent parameters do
not produce a header. Header plans are bounded to 64 fields, with 16 KiB of parameter header data.

Modern `tasks/get`, `tasks/update`, and `tasks/cancel` mirror `params.taskId` into `Mcp-Name` using
that same encoding, with the task ID unchanged in the request body.

MRTR `input_required`, `requestState`, and `inputResponses` remain opaque protocol data. The
transport does not invoke callbacks itself or automatically send another round. The daemon's
journal orchestration distinguishes intermediate receipts from completed tool actions and admits
modern tool continuations against the current receipt and unchanged binding. Each round retains
the original stable caller identity and records its own send and parent receipt before HTTP.
Explicit retries of a failed or unknown round require a trusted read-only or stable-identity
policy and unchanged round parameters. They retain the original operation and parent receipt,
append a separate retry attempt, and recheck live authority before HTTP. Transport loss never
causes an automatic POST. See the [operation journal](mcp-operation-journal.md) for retry bounds.
The [MRTR contract](https://modelcontextprotocol.io/specification/2026-07-28/basic/patterns/mrtr)
requires separate request IDs and exact state echoing; it does not grant a proxy permission to
blindly repeat an ambiguous write.

Modern `resources/read` and `prompts/get` use session-local read receipts. The receipt binds the
method and all original parameters, including client metadata and extension fields; only the progress
token can change between rounds. Opaque state is compared by digest without parsing or modification.
When no state was issued, input response IDs must identify exactly one outstanding receipt. Partial
responses and extra fields remain intact for upstream validation. Ambiguous parallel receipts, altered
parameters, invented state, and reuse of consumed receipts fail before HTTP. Reserved receipts still
participate in ambiguity checks, so concurrent admission cannot redirect another request's inputs.

Each chain permits ten requests and each session retains at most 64 active read chains, with an
8 KiB credit per chain in the shared retained-state budget. Only digests are retained. Dropping an
unsent continuation releases its reservation; beginning its send consumes the receipt even if HTTP
or delivery subsequently fails. Completion and failures release the chain. A fresh read remains
possible after a failed continuation. There is no automatic POST retry or cross-session restoration
of read state: a new activation starts a fresh read. Durable write recovery remains owned by the
operation journal. Neither kind of receipt replaces upstream namespace, principal, or state-integrity
authorization.

Only `tools/call`, `resources/read`, and `prompts/get` support modern MRTR. Control responses reject
deferred results on other methods and reject task creation; the pinned tasks extension supports
`tools/call` only. Read input requests must name a client capability family declared on that request
and also pass the integration's callback grants. Legacy requests and batches reject modern
continuation fields before sending them upstream.

Legacy nested task receipts and modern flat `resultType: "task"` receipts remain raw protocol
results. Neither receipt shape counts as a completed tool action. Task lookup uses the originating
journal receipt, current binding/catalog, and running executor. July 2026 admits `tasks/get` with
the client Tasks extension declaration; November 2025 admits `tasks/get` and `tasks/result` under
the negotiated server task capability and originating HTTP session. Earlier protocols and task
members inside March batches cannot bypass that admission path.

Each lookup uses an ordinary control slot and the shared daemon-owned HTTP drain, committing its
query send before HTTP and its factual receipt before delivery. Query errors preserve the original
pending tool outcome. A modern embedded terminal result or a legacy result fetch can settle the
original attempt without another tool send. Failed/cancelled task statuses are distinct from an RPC
error or cancellation acknowledgment.

Task cancellation shares request preparation, the ordinary control allowance, and factual response
draining with lookups, but obtains a separate cancellation permit before HTTP. A cancellation
acknowledgment never settles the modern task. A legacy cancellation requires the negotiated cancel
capability and the matching cancelled task response. The journal retains one cancellation intent
per tool attempt through errors and restart; neither automatic nor fresh-request-ID resends can
bypass it. Task queries remain available to observe the eventual outcome.

Modern task input observations persist input ID and request digests before delivery. `tasks/update`
uses the same bounded request/drain path with a distinct update permit and atomically consumed input
IDs. Partial responses pass unchanged. Stale polls, acknowledgment loss, and new activations cannot
reuse a sent input; a changed request under the same input ID records a conflict and blocks further
updates. Input/update acknowledgments remain separate from task completion. Legacy protocols reject
this modern update method. Subscribed observations use the same
[journal contract](mcp-operation-journal.md).

### Legacy task notifications

November 2025 `notifications/tasks/status` messages on tool, task, control POST responses and the
independent GET are matched against accepted task receipts using authenticated Team/actor,
server/scope/binding, protocol, and private HTTP session. Known facts commit before acquiring
provider delivery credits or checking whether the executor is still allowed to receive more work.
They cannot grant outgoing authority or overwrite the first terminal outcome.

A notification can precede its creation receipt, including across GET and POST. The stream holds
up to 64 uncorrelated messages and 8 MiB while a task-capable call is in flight, bounded by the
configured exchange timeout. Unrelated callbacks continue; held notices can therefore follow a
callback that appeared later on the wire. Receipt completion wakes the GET drain without restarting
its pinned read. Only matched facts are persisted and forwarded. Unknown/foreign handles, expired
waits, and capacity failures close provider delivery without abandoning an admitted tool result.

The [pinned legacy Tasks contract](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/cd0623765886c8cc282e3e5e1a03ab7469055fab/docs/specification/2025-11-25/basic/utilities/tasks.mdx)
keeps `completed` status distinct from the result returned by `tasks/result`. March batches reject
task notices while preserving already received tool results. Modern task notices use the separate
subscription authorization below.

### Modern subscriptions

`subscriptions/listen` opens one HTTP POST stream with the unchanged provider request. It follows
the [pinned core contract](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/cd0623765886c8cc282e3e5e1a03ab7469055fab/docs/specification/2026-07-28/basic/patterns/subscriptions.mdx)
and the released Tasks extension. Supported filters are the three core list-change flags, bounded
resource URI lists, and task ID lists. Unknown filters cannot implicitly grant access. Every filter
is checked against the binding access policy; concrete integration namespace enforcement remains
part of the open integration gate.

The first notification must acknowledge the same typed JSON-RPC ID and a subset of the requested
filters. Subsequent notifications must carry that ID and match the acknowledged filter. Task IDs
also require original journal receipts, current binding/catalog, and a running executor. List-only
subscriptions may start during authorized provider bootstrap. Task input and result facts commit
before delivery, with extensions preserved and no automatic answers to input requests.

At most eight subscriptions per proxy use the separate shared listener workspace allowance. Idle
reads hold no ordinary exchange or executor cleanup guard and recheck authority every second.
Connection establishment retains its configured timeout; stream bodies have no ordinary request
deadline. The transport never automatically repeats a subscription POST or resumes it through GET.
A correlated completion response ends the stream gracefully. An unexpected drop closes provider
transport without manufacturing completion.

Stdio cancellation closes only the matching subscription's HTTP stream locally. It does not send
`tasks/cancel` or an HTTP notification POST. Numeric and string IDs remain distinct. Stdin EOF
closes idle subscriptions while ordinary calls finish their durable drain; both kinds share the
shim's 32-exchange admission bound.

### Redaction

Transport failures contain only fixed categories or an HTTP status code. They retain no reqwest
error source, upstream URL, headers, response body, or arguments. Configured credential headers
are marked sensitive. Valid upstream MCP payloads are passed through as protocol data; they are
not copied into transport diagnostics or the operation journal.

### Activation RPC and stdio bridge

- Open accepts only an opaque server reference. Sessions are scoped to the authenticated Team,
  actor, activation, and generation; legacy mailbox tokens cannot create them. Every incoming
  message rechecks signed execution authority and `mcp:proxy` permission under the operation guard.
- MCP bootstrap may run during `starting` only after an immutable launch snapshot and local session
  are bound to the active mailbox. This admits open/close, initialization, discovery, protocol
  notifications, and registered upstream callback responses. Tools, resource reads, prompt reads,
  and task operations still require `running`. A batch qualifies for bootstrap only when every
  member qualifies independently; ordinary actor-control admission and the
  journal's prepare/send checks retain their running-only requirement. Generation, owner, lease,
  membership, and mailbox revocation apply equally to bootstrap.
- Each admitted message has a response stream. The daemon owns the HTTP operation and execution
  guard independently of that stream's receiver. Caller disconnect cannot cancel persistence of a
  factual result or release the guard while an upstream write is still running.
- An internal completion marker distinguishes normal stream completion from transport loss. The
  shim never reconstructs a lost POST. The readiness marker reports daemon protocol state without
  inferring initialization success from other members of a batch. Only MCP messages reach stdout.
- Initialization and the initialized notification are ordered through HTTP delivery. Upstream
  requests are forwarded immediately; their authenticated callback responses bypass the lifecycle
  gate to avoid initialization deadlock. Unsolicited or duplicate callback responses are rejected.
- Discovery pages preserve declarations, extensions, and cursors, while the controller maintains a
  bounded catalog for call admission. A stale response reaches its caller without replacing a
  newer catalog. A tool-list change invalidates the previous catalog until refreshed.
- Binding revocation denies future admissions. Activation cleanup removes its mounts and sessions;
  already admitted operations retain their factual journal outcomes.
- The shim rereads its credential file for each outbound RPC, pinning the activation identity and
  daemon destination while allowing token rotation. A changed identity requires a new process.
  Detached stdio threads let a failed RPC terminate the shim even when provider stdin stays open.
- Queue and concurrency limits are explicit: four tool, eight ordinary control, and eight callback
  response members per session. Callbacks use independent slots to avoid starving initialization.
  Other limits are 64 pending callbacks, 4,096 request IDs, eight response frames per RPC, and at most eight sessions
  per activation / 128 per daemon. Individual messages are bounded to 8 MiB. IDs retained for
  duplicate/callback/initialize correlation use fixed 32-byte digests; their original wire values
  remain unchanged.

### Aggregate payload budgets

One hub shares the following limits across all its sessions. Reservations are byte credits and
do not allocate that amount of memory up front.

| Pool | Default | Ownership |
| --- | --- | --- |
| Ingress | 64 MiB | Decoded RPC payload through parsing and admission |
| Working allowance | 512 MiB | Eight 64 MiB exchange reservations, covering bounded policy/HTTP/SSE/result working copies |
| Callback working allowance | 64 MiB | One independent response reservation; ordinary requests cannot consume it |
| Listener working allowance | 512 MiB | Eight independent 64 MiB reservations; one idle GET listener per admitted session |
| Delivery | 64 MiB | Journal events and RPC frames, transferring the same lease between queues |
| Retained state | 64 MiB | Discovery declaration copies, shared initialization capabilities, and read continuation digests |
| Each stdio shim | 32 MiB | Its combined incoming messages and pending stdout messages |

Acquire the working reservation before policy preparation or an upstream send. Keep it with the
daemon-owned exchange through factual completion, independently of RPC receiver lifetime. New
requests fail admission when capacity is exhausted. Delivery exhaustion closes that provider
session, while the journal keeps draining under its existing reservation and commits any observed
terminal result. It never retries the POST to recover a lost response.

A valid upstream progress message can exceed the output limit when reserialized, for example
when short exponent-form numbers expand. Treat that as lost event delivery and continue draining
the exchange; a progress encoding failure is not evidence that the tool outcome is unknown.

Byte leases follow queued data until consumer handoff, including the frame currently yielded to
the RPC encoder. Dropping a queue releases its leases. Discovery refresh resizes its existing
charge without requiring a second retained-state allocation; invalidation releases the catalog.
Initialization shares one capability value and lease across its state transition.

These are application wire-payload bounds and fixed working allowances, not an exact RSS cap.
JSON object/allocator overhead, trusted transport configuration, HTTP/gRPC implementation buffers,
and kernel buffers are outside byte accounting. Existing frame, catalog, header, correlation-count,
and session limits continue to bound their corresponding structures.

Modern tool MRTR rounds, declared retries, and task lookups use receipt-linked journal paths.
Read MRTR uses the bounded session controller; all paths retain integration access checks.

## Validation Matrix

| Boundary | Focused evidence |
| --- | --- |
| Framing | Byte-sized reads, embedded escaped newlines, malformed/unterminated/oversized input |
| SSE | Every split boundary, line endings/BOM, callbacks/progress/final results, partial event EOF |
| JSON | Dynamic tool schema/extension/cursor preservation; all three upstream error envelope forms |
| Versions | Legacy negotiated version/capabilities, initialized gate, modern metadata, versioned batching |
| Client support | Per-request isolation across concurrent HTTP requests; failed-handshake reset; roots, sampling tools, elicitation modes, logging opt-in/severity, and retired method gates; signed RPC denies unsupported inputs while retaining durable write/task facts |
| Headers | Nested mapping, absent/null handling, integer limits, unsafe/conflicting schema rejection |
| Recovery boundary | Accepted write followed by truncated response produces one POST and a transport error |
| Credentials | Trusted header reaches only the configured request; redirects are not followed; errors omit secrets |
| MRTR | Opaque state and explicit input responses survive; no automatic follow-up request |
| Real shim | Binary subprocess with gRPC and fake HTTP: initialization callback, ordered initialized delivery, paged discovery, progress/result forwarding, and credential rotation |
| RPC ownership | Dropped response stream after durable send retains the execution guard and records the actual upstream result |
| Integration access | Named resource/prompt/tool/template and callback permissions, deferred-result preservation, filtered discovery and capability projection; signed RPC proves foreign calls and subscriptions never reach HTTP, including atomic March rejection and per-member response filtering |
| Session admission | Cross-actor/activation rejection, revoked binding, cleanup, and invalid notifications without fabricated JSON-RPC replies |
| Startup | Launch/session/mailbox prerequisites; initialization callback and discovery before running; no tool send or ordinary actor control until running |
| Modern discovery | Startup RPC preserves metadata/cache hints; a real stdio probe preserves an upstream error and permits subsequent legacy initialization |
| March batches | Real shim callback/discovery/tool arrays, one POST with all sends durable, out-of-order receipts, partial-result delivery, atomic scope/ID rejection, and bootstrap without write authority |
| Legacy recovery | Two simultaneous writes use distinct GET cursors; partial batch settlement keeps original attempts; retry beyond deadline and cleared cursor cause no GET/POST |
| Listener and close | Real shim GET callbacks/resumption/DELETE; idle listener releases the executor guard; DELETE waits for durable write settlement; session 404 preserves error and requires fresh initialization |
| Failed handshake | Identified/anonymous errors, malformed results, disconnects, single/batched initialized failures, callback settlement before DELETE, reused callback IDs after retry, cleanup failure, and repeated notifications in an operating session |
| Tool MRTR | One logical operation across multiple HTTP requests, exact state echo and bound arguments, separate per-round inputs, no send for altered intent, and persisted parent-receipt links |
| MRTR retry | Explicit read/stable-identity retry after loss or error, unchanged state/inputs/identity, independent attempt records across activations, and real shim rejection of altered inputs before HTTP |
| Read MRTR | HTTP resource/prompt rounds preserve parameters and results; reject altered intent, foreign/ambiguous/consumed receipts, unsupported methods, unadvertised input families and task creation; cover unsent drop, lost response, ten-round limit, shared capacity and session isolation |
| Task lookup | Real shim preserves modern task/result envelopes and rejects foreign handles before HTTP; legacy HTTP status/result distinction, terminal failures, malformed responses, and March batch rejection |
| Task cancellation | Real shim preserves the acknowledgment, rejects a duplicate before HTTP, then records actual tool success; legacy cancel capability/status, lost acknowledgment, RPC error, malformed response, and fresh-activation resend rejection |
| Task inputs | Real shim receives unchanged elicitation input, commits response consumption before HTTP, rejects a duplicate, and later observes the tool result; HTTP partial/foreign inputs, stale polls, lost ACK, RPC errors, and changed input requests |
| Legacy task notices | Real shim receives pre-receipt notices on GET and POST, answers the intervening callback, verifies persistence before delivery, fetches the actual result, and preserves that result after a later cancellation |
| Process crash | Actual shim and a killed control-service process; file-backed reopen and daemon lock/generation reclaim; before-call, sent-without-response, parsed-success-before-commit, and durable-success checkpoints; no provider output before commit and no unknown-write replay from a new activation/RPC ID |
| Static compatibility | Fresh/resumed ACP provider starts a native stdio MCP server, initializes, discovers, and calls it; exact environment/arguments and HTTP descriptor fields retained, HTTP capability filtering, and no static-loader invocation in either loop launch path |
| Modern subscriptions | HTTP acknowledgment/filter/order checks, typed IDs, local cancellation, idle authority revocation, capacity, and signed-RPC/shim task input/result delivery plus EOF with an idle subscription |

## Operational Notes

The current tests exercise real loopback HTTP servers and raw TCP truncation. They require local
socket permission. They do not require a personal Mem account or a paid provider.

The journaled client keeps draining after losing its event receiver and stores a factual terminal
result before returning it. An authenticated daemon task fixture verifies ownership and the
execution guard across caller disconnect; startup journal recovery is wired. Real shim fixtures
cover signed streaming RPCs, callback/result queues, and per-message credential refresh. The hub
starts empty and configured local activations add their mounts during launch. The configured ACP
fixture additionally checks inherited provider/shim environments and an actual journaled HTTP call.
The process-crash fixture uses the actual shim and production RPC/journal implementations in an
isolated test service process. It kills that process without cleanup, terminates its provider-side
shim, and only then releases the old executor reservation. Its checkpoints are before a tool call,
after upstream reception while the response is withheld, after parsing success but before committing
it, and after committed success with unread provider output. The uncommitted checkpoint uses a TEMP
SQLite trigger and an update hook on the sole test connection. It pauses only when an attempt changes
to `succeeded`, proves the old committed state remains `sent` and provider output is empty, then kills
the process. Reopen rolls back that update and recovers the write as unknown. Database child-exit
tests separately cover committed prepared/sent/completed states.

## Open Risks

- Unknown extension capabilities remain opaque and require integration-specific rules before
  enabling new surfaces. Observed deferred tool receipts block replay of the original write until
  a linked continuation or lookup establishes the outcome.
- Effective scope currently uses the canonical configured endpoint and Team space. Endpoint
  aliases or moves need explicit reconciliation of outstanding writes; changing an endpoint is
  not evidence that retrying an unresolved write is safe.
- Cursors are not persisted across daemon restart; recovery cannot claim replayed stream history
  after losing that transient state.
- The common access policy enforces declared surface grants. Mem namespace authorization, including
  tools without a declared space field, still needs completion. The temporary tool-only Mem policy
  must not be treated as a completed alternative to full scoped integration. Upstream routing
  metadata cannot establish resource/prompt or namespace authority.

## Source Journals

- [Shared MCP proxy checkpoint](../journal/2026-09-15-shared-mcp-proxy.md)
