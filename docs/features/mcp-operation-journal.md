# MCP Operation Journal

## Problem

An upstream write can take effect before its response reaches an agent. A new activation, a new
request number, or a daemon restart does not prove that repeating that write is safe. The local
control store must retain the send boundary independently of provider and transport lifetime.

## Scope

One journal serves the trusted local MCP proxy for Mem and registered apps. It records operation
intent, individual send attempts, and ordered receipt events. The store, trusted call preparation,
and journaled [HTTP transport](mcp-proxy-transport.md) are implemented. Daemon startup reconciles
earlier sends before starting runtime services. Signed MCP streaming RPCs and a local stdio shim
exercise this path. Configured Mem bindings and provider environment isolation are connected to
local ACP launch; complete protocol orchestration and integration authorization remain incomplete.

## Non-Goals

- Storing tool arguments, result bodies, raw errors, URLs, credentials, or caller identity values.
- Claiming exactly-once execution at an upstream service without an upstream identity contract.
- Replacing canonical tasks, activation outcomes, or provider history.
- Granting execution permission through a journal receipt or caller-supplied activation ID.

## Architecture

The agent domain owns safe intent and receipt types. `agenthub-db::mcp_operations` owns additive
control tables for operations, attempts, and events. Existing Mem status/error types alias the
shared domain types. There is no separate Mem or app journal implementation.

The daemon constructs an intent only after resolving the binding and validating discovered tool
policy. It prepares the operation, commits a send attempt, then sends upstream using the returned
process-local permit. No database transaction spans network I/O. The permit is not serializable.

`agenthub-mcp::policy` constructs a non-cloneable `PreparedToolCall` containing both the actual
owned HTTP request and its intent. The integration supplies the trusted effective scope, binding
revision, replay declarations, and argument-binding callback. Preparation uses the exact discovered
schema, applies scope before hashing, and validates any declared stable identity at its static
schema property path. It does not infer write replay permission from upstream hints.

`JournaledMcpClient` drains the actual exchange after committing `sent`. It persists a matching
result before returning the raw response. Losing an event receiver does not stop that drain or
discard a later result. Runtime callers must own the future in the daemon task group and retain the
execution operation guard until it settles. The signed streaming RPC and real shim fixtures
exercise this ownership boundary through provider disconnects.

## Contracts

### Identity and authority

- A request key is unique within a Team and actor. Reusing it with different intent is rejected.
- Intent fixes the profile reference, effective upstream scope, binding revision, tool/schema,
  argument digest, semantic request digest, and replay policy. Digests contain no raw arguments or
  identity values. The request digest omits the JSON-RPC ID and progress correlation token, while
  retaining other parameters and extensions. An older intent without this optional digest remains
  readable but cannot silently match a newly prepared intent with different fields.
- Effective scope identifies the upstream authorization/data boundary, independent of a profile
  alias. Configuration revision and scope identity have different purposes.
- The trusted policy derives stable-identity request keys from the original declared caller
  identity. An arbitrary MCP JSON-RPC request number is only a transport correlation value.
- Prepare and send independently verify the current daemon generation and existing live loop
  executor fence, current Team membership, running activation, and active mailbox partition.
  The separate MCP protocol-bootstrap admission during `starting` never grants a journal send
  permit. A tool call remains subject to the running-only journal check.
- Binding authorization and revocation must also be checked by the proxy before each send. The
  database journal does not replace that integration-specific authority check.
- Canonical hashes sort nested objects and normalize equivalent integer spellings such as `1`,
  `1.0`, and `1e0`. Exact large integer values are not rounded through floating-point conversion.
- Discovery snapshots are bounded to 1,024 tools and 8 MiB. Tool names, schemas, ordering, and
  extensions of approved tools survive unchanged. Invalid July 2026 HTTP header annotations exclude that tool;
  legacy versions retain the annotations as uninterpreted data. The proxy applies trusted access
  grants before storing or advertising the catalog. Rejected single/batch/subscription requests
  cannot acquire send authority from discovery or protocol metadata.

### Send and recovery

```text
prepared -> sent -> succeeded
                 -> failed
                 -> outcome_unknown
```

`prepared` means no send permit has been issued. Its original intent can be used by a later valid
activation. `sent` is committed before any request bytes are sent. The journal sets SQLite
`synchronous=FULL` on each acquired write connection before starting the transaction. That stronger
setting remains on the connection when it returns to the shared pool; transcript-store defaults
are unchanged. A commit error never authorizes a send.

Each attempt has a distinct private permit and an increasing number. Concurrent requests cannot
receive a permit for the same attempt. Failure or loss after issuing a permit is conservatively
unknown unless a valid upstream result establishes success or failure.

A March batch owns one bound HTTP request and a separate intent for every tool member. All send
transitions commit in one transaction under the same live executor and daemon checks. Rejection
of any member rolls back every send transition and attempt; earlier prepared rows carry no send
authority. Duplicate operation IDs and conflicting unresolved effects are rejected before HTTP.
Each matching response completes its own permit before delivery, including out-of-order arrays.
An incomplete batch preserves known receipts and marks only unresolved members unknown.

Legacy SSE recovery retains the original permit while using GET with the exact originating stream
cursor. It creates neither a second operation nor another send attempt. Each recovered factual
result is persisted before provider delivery. Session DELETE waits for admitted exchanges to settle;
an upstream 404 requires a new protocol session without authorizing a replay of the original write.

After claiming a new daemon generation, startup recovery marks old-generation `sent` attempts
unknown in bounded batches. Opening or migrating a database alone never recovers live sends.
Recovery is not proof that an old request did not take effect.

An `input_required` result or an asynchronous task receipt records a typed `Deferred` completion
and response digest with `outcome_unknown` status: the RPC response is known, but the tool's final
outcome is not. This is neither a failure nor successful completion. A later transport-loss report
cannot overwrite that observed receipt. Even a read or stable-identity call cannot replay its
initial request from this state. Modern tool continuations and task lookups use separate
receipt-bound send paths.

Task receipts include both the legacy nested `task` object and the modern Tasks extension's
flat `resultType: "task"` shape. The latter is defined by the
[released extension schema](https://github.com/modelcontextprotocol/ext-tasks/blob/9263312d11a682ac83f83fe84794d4627efd22f5/schema/2026-07-28/schema.ts).
An initial handle remains deferred even when it reports a terminal task status; only a linked
result lookup can establish the tool outcome. The full receipt is forwarded unchanged. The journal
records only digests and typed protocol/session correlation facts, without raw task IDs or status text.

### Asynchronous task lookups

A valid task acceptance records an immutable link to the originating tool attempt. Lookup admission
requires that recorded handle digest, the same Team, actor, effective scope, binding, protocol version,
and unchanged discovered tool schema. Legacy tasks also retain the originating HTTP session digest.
A new activation can query a modern task under current authority. A different legacy HTTP session
cannot silently adopt the old handle. Old receipts without correlation metadata remain readable
but cannot authorize a lookup.

Each caller-initiated query commits its own send record with SQLite `synchronous=FULL` before HTTP.
It does not create a new tool attempt or repeat the original write. The current daemon and live
executor are checked at admission, with a fresh RPC request key and at most 4,096 query records per
operation. Authorized inspection uses bounded pages with a stable sequence cursor. The task link
survives terminal settlement so the caller can fetch the actual upstream result again; a journal
digest cannot reconstruct that result.

For July 2026, `tasks/get` returns the detailed task and an embedded result when completed. The
embedded tool result, including `isError`, determines the original operation's outcome. For November
2025, `tasks/get` reports status and `tasks/result` retrieves the tool result. A legacy `completed`
status alone does not establish tool success. A valid task failure or cancellation status records a
typed failure; a cancellation acknowledgment is not such a status observation.

Query RPC errors and transport failures are query receipts only and leave the tool pending. A
matching terminal fact settles the original attempt and its operation atomically before provider
delivery. An admitted query can record a late fact after executor shutdown. The first terminal fact
wins; later conflicting responses remain inspectable on their own query records without overwriting
the operation or a newer attempt. Daemon recovery marks interrupted queries unknown independently
of the original task receipt. Queries are never automatically repeated.

Callers retain actual task handles and observe upstream polling and retention guidance; the journal
neither polls on their behalf nor extends server retention.

### Task notification observations

Modern task subscriptions require the original task receipt, Team/actor, binding, scope, protocol,
and discovered schema checks used by lookups. Each admitted handle gets a private observation
permit that cannot authorize a tool send. Only task IDs included in both the request and the
server acknowledgment may deliver observations.

Each distinct notification commits its digest, typed outcome, and input receipt updates in one
FULL-synchronous transaction before delivery. Subscription IDs are excluded from notification
identity, so reconnects cannot duplicate history or release consumed inputs. History is bounded to
4,096 observations per task attempt and supports scoped sequence pages. Changed input meanings
retain the existing persistent conflict behavior.

An admitted notification can commit a received fact after executor shutdown. First terminal facts
win across polls, notifications, and cancellation responses; later observations retain their own
receipts without replacing that result or a newer attempt. A subscription acknowledgment or
graceful stream closure never completes the tool.

Legacy November 2025 status notifications use the authenticated proxy owner's Team/actor and the
captured server binding, protocol version, and private HTTP session to find an immutable accepted
task receipt. A current discovery catalog is not needed to record a fact about an already admitted
task; outgoing queries still require current authority and schema checks. Unknown or ambiguous
handles cannot settle or disclose another operation. The observation capability grants no send.

A status may arrive on a POST response or the independent GET before its creation receipt. While
an eligible tool call is in flight, each stream can retain at most 64 such messages within 8 MiB
for the configured exchange timeout. Callbacks continue while these uncorrelated messages wait.
After the creation receipt commits, pending facts commit before delivery. Without a matching
receipt, expiry or capacity loss rejects provider delivery. Pending raw messages are transient;
daemon restart does not claim they were accepted. Legacy `completed` remains status-only until
`tasks/result` supplies the actual tool result; failed/cancelled statuses can settle the task.

### Task cancellation

`tasks/cancel` requires a recorded task on the current deferred tool attempt and the same live
authority checks as a lookup. Cancellation has its own additive durable intent/receipt table.
The send commits with `synchronous=FULL` before HTTP. At most one cancellation is sent per task
attempt, including after a lost acknowledgment, RPC error, fresh activation, or daemon restart.
The journal cannot infer that an unsuccessful cancellation had no effect; further action requires
upstream reconciliation. This limit does not prevent subsequent task queries.

July 2026 cancellation returns an acknowledgment, which records only the cancellation RPC outcome.
The tool remains pending and may ultimately succeed even after that acknowledgment. November 2025
requires the negotiated `tasks.cancel` capability and a response identifying the same task in the
`cancelled` state. That observed status can settle the original attempt as a typed cancellation.
Neither version repeats the original tool send. `notifications/cancelled` is not a task cancellation.

Restart recovery marks an interrupted cancellation unknown without erasing its sent intent or
changing the parent task. A process-local permit can record a late factual response after executor
shutdown. A conflicting task result already recorded by another lookup takes precedence, with the
late cancellation receipt retained for inspection. Cancellation inspection requires Team and actor
scope and returns typed facts only.

### Task input responses

A modern `tasks/get` or subscribed task observation with `input_required` commits the input IDs and request payload
digests before the provider receives that observation. Input requests retain their ordinary
elicitation, sampling, or roots semantics and are delivered unchanged; the proxy never answers
them itself. At most 64 inputs are accepted in one observation and 4,096 distinct IDs per task
attempt. Repeated observations retain the first request digest and any prior send association.

`tasks/update` requires the same recorded task and current authority as its lookup. Each supplied
input ID must already be known and unused. A single FULL-synchronous transaction records the update
send and consumes all of its input IDs; rejection of any member rolls back the whole update. Partial
sets are allowed. Unknown or already-used IDs cannot authorize a send. The caller retains and
supplies the actual input responses, while only their digests enter the journal. Old polls cannot
release a consumed input, and an input response lost after sending cannot be repeated under a new
request ID or activation. An update acknowledgment, RPC error, or transport loss is its own receipt
and never completes the original tool attempt.

The server must keep each input ID's meaning fixed for the task lifetime. A changed request payload
under an observed ID records a persistent conflict and prevents that observation from reaching the
provider. All further updates for that task are denied, including after reopen; the original
request digest remains inspectable. Delayed responses to previously observed, unused IDs remain
subject to the upstream's current outstanding-input state. The proxy does not reconstruct that
state from polling order.

Update admission is denied after a cancellation intent or a terminal tool outcome. Each operation
allows at most 4,096 update sends. Input and update inspection uses Team/actor-scoped sequence
pages. Daemon recovery marks interrupted updates unknown while retaining consumed input IDs.
Admitted updates may record late acknowledgments after executor shutdown, without changing a
tool result already established by a task lookup or subscribed notification.

### Multi round-trip tool calls

For the [2026-07-28 MRTR pattern](https://modelcontextprotocol.io/specification/2026-07-28/basic/patterns/mrtr),
the journal stores only digests of the opaque state, server input IDs, and response ID alongside
the observed deferred receipt. The provider retains the actual state and input results. A supplied
`requestState` or `inputResponses` selects continuation admission rather than ordinary replay.

Admission requires the same Team, actor, effective scope, binding, tool schema, bound arguments,
semantic base parameters, and replay policy. It resolves one current deferred receipt and checks
the exact state value or its absence. Without state, a response must match at least one issued
input ID (or both maps must be empty). Server input IDs distinguish parallel matching receipts;
ambiguous matches fail before HTTP. Other missing or additional input values remain upstream validation
concerns and are forwarded unchanged. No opaque state is decoded or reconstructed by the proxy.

The linked send and its parent receipt digest commit atomically with a new attempt on the existing
logical operation. Every send rechecks the current daemon and live executor. A new RPC ID is
required; concurrent consumers receive at most one permit. The original stable identity remains
unchanged on the wire, while each round has its own request digest. A chain allows ten continuation
rounds and at most 64 input IDs per receipt. These are proxy resource limits, not MCP defaults.

`mcp_operation_continuations` is an additive table linking each new attempt to its previous attempt
and known response digest. Attempt inspection exposes those links without raw state or answers.
Old deferred receipts without correlation metadata remain readable and cannot grant continuation
authority. Reopening the journal preserves the links and can resolve a caller-supplied state
against the receipt after a fresh proxy opens; current binding and execution authority still apply.

A final result completes the same logical operation. A lost continuation result leaves that
attempt unknown; neither its continuation POST nor the initial POST is automatically resent.
The caller can explicitly retry the current failed or unknown round only when trusted binding
policy declares the entire tool read-only or provides a stable identity. An upstream hint alone
does not permit a retry. Admission requires the exact semantic parameters of that round, including
opaque state and all input responses, along with the unchanged base intent and original identity.
Only the RPC ID and delivery-only progress token can change. The caller retains and resubmits the
actual payload; the journal cannot reconstruct it.

An additive `mcp_operation_continuation_retries` table links each retry attempt to that round's
first send. The original input-required receipt is consumed once and its response digest remains
visible on retry inspection. Retries do not overwrite earlier failed/unknown attempts or become
new rounds. Each round permits at most three retries, independently of the ten-round bound.
The new send and retry link commit atomically after live daemon/executor checks. Concurrent
callers receive at most one permit; stale RPC IDs, changed parameters, non-idempotent operations,
and ambiguous receipt matches cannot send. Reopen and new activations retain these constraints.
A subsequent input-required result starts the next round from the retry that observed it.
An asynchronous task handle returned by a round follows the task lookup contract above.

The original permit can record a factual response after activation expiry, cancellation, or
cleanup. It can resolve an unknown attempt if no replacement attempt exists. It cannot complete a
newer attempt. Ordered events preserve the earlier uncertainty when a late result resolves it.

### Replay

- Reads can retry with a new attempt. Successful operations retain a receipt and are not resent
  under the same request key.
- Unknown or failed writes can retry only with a declared stable identity, unchanged intent, and
  the original identity value. `idempotentHint` alone is insufficient.
- MCP errors, JSON-RPC errors, and error envelopes are known failure receipts; none proves that a
  write had no side effect. Non-idempotent writes are not automatically retried after those errors.
- A new request key cannot bypass an outstanding write against the same Team, effective scope,
  tool, and argument digest. The check covers other actors and profile aliases, and runs again at
  send time to handle operations prepared concurrently. Changing schema/configuration revisions
  does not conceal an earlier unresolved write.
- A stable identity cannot be reused through another request key, payload, or actor. The original
  operation must be resolved. The proxy must preserve the original schema field and value on retry.
- A later intentional non-idempotent write may use a new request key after the prior operation has
  succeeded. The journal does not treat every identical future payload as a duplicate.

### Inspection and redaction

Operation reads and bounded attempt/event pages require explicit Team and actor scope. Event IDs
provide stable ordering across restarts; each attempt records its actual activation and generation,
while the operation retains its original activation. Product endpoints must apply their normal
inspection authorization before using these store methods.

Completion records contain typed result/error categories, response digests, and bounded digest-only
input-receipt correlation facts. Raw
upstream responses are not replayable from this store. A known receipt must not be presented as a
reconstructed tool result. Transport code must preserve the real response while it is available.

## Validation Matrix

| Boundary | Evidence |
| --- | --- |
| Additive migration and reopen | Existing agent configuration survives; repeated migration preserves live sends |
| Send durability | WAL/FULL checked on the actual transaction connection; child exits without SQLite cleanup after each commit boundary |
| Unknown writes | Restart, new request IDs, new actors, and profile/schema changes cannot bypass unresolved writes |
| Stable identity | Immutable intent, distinct attempts, no stale completion of a later attempt |
| Completion versus authority | Original permit records a late result after executor cleanup; new sends fail |
| Concurrency | Concurrent preparation converges; only one send permit; prepared duplicates recheck before send |
| Scope and history | Membership/lease/daemon checks and scoped, ordered, bounded event/attempt queries |
| Redaction | Intent types reject raw payload/credential fields and invalid digest/reference forms |
| Real send boundary | Fake upstream observes committed `sent` before reading the request; actual scoped arguments match the journal digest |
| Lost result | A consumed write with an incomplete response remains unknown across database reopen and a fresh activation |
| Real stable retry | Original operation receives a second attempt with the same caller identity and parameters under a new RPC ID |
| Provider disconnect | Authenticated daemon task retains the execution guard after caller cancellation and journals the late HTTP result |
| Control process crash | Real shim/RPC/HTTP path killed with a file-backed journal, including a paused success update before commit; no premature provider output, exactly-once recovery of sent attempts, preserved committed success, rejected old credentials, and no unknown-write replay from a new activation/RPC ID |
| Deferred response | Raw input/task receipt retained transiently; typed receipt survives loss and blocks initial-request replay |
| MRTR | Additive migration and reopen, atomic linked sends, current intent/state/executor checks, fresh RPC IDs, bounded rounds, unchanged HTTP inputs, final settlement, and lost-round replay rejection |
| MRTR retry | Additive retry migration, exact round digest and stable identity, fresh activation and daemon checks, concurrent admission, preserved attempt/parent links, three retries per round, and subsequent rounds after a retry |
| Batch sends | Atomic rollback on a stale/conflicting member; one POST; out-of-order completion; partial-result uncertainty and replay rejection across activations |
| Task lookup | Additive migration, authority/session/schema checks, legacy receipts without metadata rejected, concurrent admission, query-only restart recovery, terminal result settlement, and first-fact preservation without another tool send |
| Task cancellation | Additive migration, exactly one concurrent send, acknowledgment versus terminal status, restart without resend, late/conflicting facts, scoped inspection, and tool queries after cancellation |
| Task inputs | Input/update migration, atomic partial consumption, concurrent updates, unchanged wire payloads, key equivocation retained across reopen, stale polls, lost acknowledgment, fresh activations, and real shim input/update/result flow |
| Legacy task notices | Authenticated owner, binding/protocol/session matching, receipt races across POST/GET, callbacks during the race, bounded pending data, late facts, delivery loss, and status-only completion |
| Task subscriptions | Receipt authorization, notification migration/deduplication, bounded scoped history, input consumption/conflicts, late settlement and first-terminal-fact preservation; actual shim delivery follows committed facts |

## Operational Notes

The HTTP client has a bounded request deadline and never automatically repeats a POST. The daemon
startup path completes journal recovery before starting runtime services. The journaled client
reports lost event delivery separately from the factual tool result, so a controller can close a
broken provider stream without abandoning a send. Configured local activations use the authenticated
proxy; complete integration authorization remains a separate acceptance gate.

## Open Risks

- The proxy enforces trusted surface grants; integration adapters still need complete upstream
  namespace authorization, capability handling, and endpoint-alias reconciliation. Non-tool continuations remain incomplete.
  Configured Mem launch fixtures establish provider credential/environment isolation for that path.
- An upstream service must honor its declared stable identity for a retry to be safe.
- Retained ambiguous non-idempotent writes need explicit upstream reconciliation; changing
  configuration or deleting history is not a recovery mechanism.
- Filesystem/storage durability still depends on the platform honoring SQLite synchronization.

## Source Journals

- [Shared MCP proxy implementation checkpoint](../journal/2026-09-15-shared-mcp-proxy.md)
