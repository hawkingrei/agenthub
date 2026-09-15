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
  extensions survive unchanged. Invalid July 2026 HTTP header annotations exclude that tool;
  legacy versions retain the annotations as uninterpreted data. Pagination, refresh sequencing,
  and integration-specific catalog authorization remain controller responsibilities.

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
initial request from this state. Modern tool continuations use a separate receipt-bound send path;
asynchronous task-result resolution remains pending.

Task receipts include both the legacy nested `task` object and the modern Tasks extension's
flat `resultType: "task"` shape. The latter is defined by the
[released extension schema](https://github.com/modelcontextprotocol/ext-tasks/blob/9263312d11a682ac83f83fe84794d4627efd22f5/schema/2026-07-28/schema.ts).
An initial handle remains deferred even when it reports a terminal task status; only a linked
result lookup can establish the tool outcome. The full receipt is forwarded unchanged while its
digest, rather than the task ID or status text, is recorded in the journal.

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
Resolving asynchronous task handles remains follow-up controller work.

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
| Deferred response | Raw input/task receipt retained transiently; typed receipt survives loss and blocks initial-request replay |
| MRTR | Additive migration and reopen, atomic linked sends, current intent/state/executor checks, fresh RPC IDs, bounded rounds, unchanged HTTP inputs, final settlement, and lost-round replay rejection |
| MRTR retry | Additive retry migration, exact round digest and stable identity, fresh activation and daemon checks, concurrent admission, preserved attempt/parent links, three retries per round, and subsequent rounds after a retry |
| Batch sends | Atomic rollback on a stale/conflicting member; one POST; out-of-order completion; partial-result uncertainty and replay rejection across activations |

## Operational Notes

The HTTP client has a bounded request deadline and never automatically repeats a POST. The daemon
startup path completes journal recovery before starting runtime services. The journaled client
reports lost event delivery separately from the factual tool result, so a controller can close a
broken provider stream without abandoning a send. A full provider-facing proxy is not yet enabled.

## Open Risks

- Integration adapters still need to derive the effective scope from actual configured authority,
  check binding revocation at every call, manage discovery refresh and protocol continuations, and
  prove provider credential/environment isolation through the real stdio shim and ACP launch.
- An upstream service must honor its declared stable identity for a retry to be safe.
- Retained ambiguous non-idempotent writes need explicit upstream reconciliation; changing
  configuration or deleting history is not a recovery mechanism.
- Filesystem/storage durability still depends on the platform honoring SQLite synchronization.

## Source Journals

- [Shared MCP proxy implementation checkpoint](../journal/2026-09-15-shared-mcp-proxy.md)
