# MCP Operation Journal

## Problem

An upstream write can take effect before its response reaches an agent. A new activation, a new
request number, or a daemon restart does not prove that repeating that write is safe. The local
control store must retain the send boundary independently of provider and transport lifetime.

## Scope

One journal serves the trusted local MCP proxy for Mem and registered apps. It records operation
intent, individual send attempts, and ordered receipt events. The store is implemented; proxy
transport, credential delivery, and launch integration remain the next implementation stage.

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

## Contracts

### Identity and authority

- A request key is unique within a Team and actor. Reusing it with different intent is rejected.
- Intent fixes the profile reference, effective upstream scope, binding revision, tool/schema,
  argument digest, and replay policy. Digests contain no raw arguments or identity values.
- Effective scope identifies the upstream authorization/data boundary, independent of a profile
  alias. Configuration revision and scope identity have different purposes.
- The trusted policy derives stable-identity request keys from the original declared caller
  identity. An arbitrary MCP JSON-RPC request number is only a transport correlation value.
- Prepare and send independently verify the current daemon generation and existing live loop
  executor fence, current Team membership, running activation, and active mailbox partition.
- Binding authorization and revocation must also be checked by the proxy before each send. The
  database journal does not replace that integration-specific authority check.

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

## Operational Notes

The proxy must settle issued attempts independently of provider disconnects, with a bounded
deadline. It must classify transport uncertainty explicitly and finish startup recovery before
accepting new proxy work. These transport/runtime integrations are pending. A journal-only
checkpoint does not enable writable tools.

## Open Risks

- The trusted policy still needs protocol fixtures proving canonical digests, effective scope
  construction, stable identity discovery, unchanged wire payloads, and credential isolation.
- An upstream service must honor its declared stable identity for a retry to be safe.
- Retained ambiguous non-idempotent writes need explicit upstream reconciliation; changing
  configuration or deleting history is not a recovery mechanism.
- Filesystem/storage durability still depends on the platform honoring SQLite synchronization.

## Source Journals

- [Shared MCP proxy implementation checkpoint](../journal/2026-09-15-shared-mcp-proxy.md)
