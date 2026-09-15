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
provider-facing bridge. Configured bindings, launch descriptors, the remaining protocol controller,
and provider environment isolation remain in progress.

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
Configured mount resolution and ACP launch registration remain required before production use.

## Contracts

### Version and lifecycle

| Version | Transport behavior |
| --- | --- |
| 2025-03-26 | Initialize handshake, optional HTTP session, GET stream/resumption, JSON-RPC batches |
| 2025-06-18 and 2025-11-25 | Initialize handshake, optional HTTP session, GET stream/resumption, single-message envelopes |
| 2026-07-28 | Per-request version/client metadata, standard/custom request headers, request-scoped SSE and MRTR results |

The legacy lifecycle adopts the upstream's supported version and unchanged capabilities. Tools
cannot start before initialization and the initialized notification. An upstream initialization
error remains an error; the session does not manufacture a successful handshake. Unknown versions
fail explicitly. Modern requests do not receive a synthetic legacy initialization response.

The protocol boundary follows the primary [2025-03 transport](https://modelcontextprotocol.io/specification/2025-03-26/basic/transports),
[2025-11 transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports), and
[2026-07 transport](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http)
contracts. The modern [versioning contract](https://modelcontextprotocol.io/specification/2026-07-28/basic/versioning)
differs from legacy handshake negotiation; the adapter must preserve this distinction.

### HTTP and JSONL

- JSON-RPC methods, IDs, extension fields, schemas, result content, and error data remain unchanged.
  JSON serialization can change whitespace or object member ordering.
- Each message is one JSONL line. Reads reject unterminated or malformed frames and enforce an
  8 MiB message limit before growing a line buffer beyond the limit.
- HTTP POST accepts JSON or SSE. Legacy notifications/responses require an empty 202 acknowledgment.
  Valid JSON-RPC error bodies remain available even when carried by an HTTP error status.
- Automatic HTTP retries and redirects are disabled. Ambient proxy variables are not consulted by
  this client; upstream routing and credentials come from the trusted configuration.
- Header construction and size/type validation happen before a request can be journaled as sent.
- Requests have an explicit deadline. EOF before a terminal response does not establish a tool
  outcome; the journal/controller must classify that boundary as uncertain.
- Legacy stream resumption constructs GET with the exact cursor. It never reconstructs a tool POST.
  Retry timing and cursor lifetime belong to the session controller, not an automatic transport retry.
- The SSE decoder accepts LF/CRLF/CR, a UTF-8 BOM, comments, multiline data, empty priming events,
  cursor updates, and retry hints. It does not dispatch an unterminated event on EOF.
- Legacy SSE can carry server requests and notifications. Modern SSE rejects independent server
  requests; their modern representation is an MRTR result.

### Modern metadata

`MCP-Protocol-Version` must match the request metadata. Standard method/name headers and discovered
`x-mcp-header` parameter mappings are generated without changing tool arguments. Nested mappings
must be statically reachable through `properties`. Invalid paths, duplicate header names, forbidden
types, and unsafe names invalidate a tool's header plan. Discovery must exclude such tools when
using HTTP, as required by the protocol.

Header values use the specified Base64 sentinel for non-ASCII/control characters, surrounding
whitespace, or a literal sentinel-shaped value. Primitive string/boolean/integer values retain
their meaning. Integer headers must fit the JavaScript safe range. Null or absent parameters do
not produce a header. Header plans are bounded to 64 fields, with 16 KiB of parameter header data.

MRTR `input_required`, `requestState`, and `inputResponses` remain opaque protocol data. The
transport does not invoke callbacks itself or automatically send another round. The daemon's
journal orchestration still needs to distinguish a known intermediate response from a completed
tool action and preserve the original stable caller identity across permitted continuation rounds.
The [MRTR contract](https://modelcontextprotocol.io/specification/2026-07-28/basic/patterns/mrtr)
requires separate request IDs and exact state echoing; it does not grant a proxy permission to
blindly repeat an ambiguous write.

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
  task operations, and batches still require `running`; ordinary actor-control admission and the
  journal's prepare/send checks retain their running-only requirement. Generation, owner, lease,
  membership, and mailbox revocation apply equally to bootstrap.
- Each admitted message has a response stream. The daemon owns the HTTP operation and execution
  guard independently of that stream's receiver. Caller disconnect cannot cancel persistence of a
  factual result or release the guard while an upstream write is still running.
- An internal completion marker distinguishes normal stream completion from transport loss. The
  shim never reconstructs a lost POST. Only MCP messages are written to stdout.
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
- Queue and concurrency limits are explicit: four tool and eight control requests per session,
  64 pending callbacks, 4,096 request IDs, eight response frames per RPC, and at most eight sessions
  per activation / 128 per daemon. Individual messages are bounded to 8 MiB. An aggregate byte
  budget across concurrent sessions remains a production integration requirement.

The raw transport version table does not imply complete controller support. March 2025 batches,
legacy GET listening/resumption and upstream DELETE, linked MRTR/task rounds, modern server
discovery, and integration-specific authorization for non-tool methods remain controller work.

## Validation Matrix

| Boundary | Focused evidence |
| --- | --- |
| Framing | Byte-sized reads, embedded escaped newlines, malformed/unterminated/oversized input |
| SSE | Every split boundary, line endings/BOM, callbacks/progress/final results, partial event EOF |
| JSON | Dynamic tool schema/extension/cursor preservation; all three upstream error envelope forms |
| Versions | Legacy negotiated version/capabilities, initialized gate, modern metadata, versioned batching |
| Headers | Nested mapping, absent/null handling, integer limits, unsafe/conflicting schema rejection |
| Recovery boundary | Accepted write followed by truncated response produces one POST and a transport error |
| Credentials | Trusted header reaches only the configured request; redirects are not followed; errors omit secrets |
| MRTR | Opaque state and explicit input responses survive; no automatic follow-up request |
| Real shim | Binary subprocess with gRPC and fake HTTP: initialization callback, ordered initialized delivery, paged discovery, progress/result forwarding, and credential rotation |
| RPC ownership | Dropped response stream after durable send retains the execution guard and records the actual upstream result |
| Session admission | Cross-actor/activation rejection, revoked binding, cleanup, and invalid notifications without fabricated JSON-RPC replies |
| Startup | Launch/session/mailbox prerequisites; initialization callback and discovery before running; no tool send or ordinary actor control until running |

## Operational Notes

The current tests exercise real loopback HTTP servers and raw TCP truncation. They require local
socket permission. They do not require a personal Mem account or a paid provider.

The journaled client keeps draining after losing its event receiver and stores a factual terminal
result before returning it. An authenticated daemon task fixture verifies ownership and the
execution guard across caller disconnect; startup journal recovery is wired. Real shim fixtures
cover signed streaming RPCs, callback/result queues, and per-message credential refresh. Production
startup currently installs an empty binding set, so these fixtures do not establish ACP/Mem launch.

## Open Risks

- Configured bindings and provider launch remain unwired.
- Linked MRTR continuation/task-result admission remains controller work. Observed deferred receipts already block a replay
  of the original request without claiming a final tool outcome.
- Existing static MCP configuration and provider environment isolation need real shim/ACP fixtures.
- Legacy GET cursors must be scoped to the exact session and stream when recovery is wired.
- Production bindings must intersect all advertised capabilities and non-tool methods with their
  approved scope. Generic protocol forwarding alone does not establish resource/prompt authority.

## Source Journals

- [Shared MCP proxy checkpoint](../journal/2026-09-15-shared-mcp-proxy.md)
