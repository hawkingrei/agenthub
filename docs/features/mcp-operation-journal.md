# MCP Operation Journal

## Problem

An upstream write can take effect before its response reaches an agent. A new activation, a new
request number, or a daemon restart does not prove that repeating that write is safe. The local
control store must retain the send boundary independently of provider and transport lifetime.

## Scope

One journal serves the trusted local MCP proxy for Mem and registered apps. It records operation
intent, individual send attempts, and ordered receipt events. The store, trusted call preparation,
and journaled [HTTP transport](mcp-proxy-transport.md) are implemented. Daemon startup reconciles
earlier sends before starting runtime services. The provider-facing MCP RPC/shim, binding resolver,
credential delivery, and launch wiring remain incomplete.

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
execution operation guard until it settles; the authenticated daemon integration fixture exercises
this ownership seam. The MCP RPC surface itself is still pending.

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

After claiming a new daemon generation, startup recovery marks old-generation `sent` attempts
unknown in bounded batches. Opening or migrating a database alone never recovers live sends.
Recovery is not proof that an old request did not take effect.

An `input_required` result or an asynchronous task receipt records a typed `Deferred` completion
and response digest with `outcome_unknown` status: the RPC response is known, but the tool's final
outcome is not. This is neither a failure nor successful completion. A later transport-loss report
cannot overwrite that observed receipt. Even a read or stable-identity call cannot replay its
initial request from this state. A linked continuation/task-result controller must resolve the
receipt; that controller remains pending, and ordinary call preparation rejects caller-supplied
`requestState` or `inputResponses` instead of bypassing this boundary.

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

Completion records contain only typed result/error categories and optional response digests. Raw
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
