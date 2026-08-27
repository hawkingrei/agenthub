# Team Runtime Delivery Receipts

## Problem

Team mailbox rows are durable, but the prompt that wakes a running actor was previously best-effort.
A daemon restart or transient runtime-input failure could leave a pending mailbox message without a
corresponding retry. The mailbox row's `delivered` state cannot solve this because it records explicit
consumer acknowledgement, not submission to an agent runtime.

## Scope

- Durable receipts for direct-message, coordinator-mention, and permission-review runtime hints.
- Stable delivery identity across retries and daemon restarts.
- Lease-based claiming, bounded exponential retry, and stale-attempt fencing.
- Runtime session attribution after the input transport accepts a hint.

## Non-Goals

- Treating runtime submission as mailbox acknowledgement or completed actor work.
- Exactly-once execution inside an external provider after a process crash.
- Persisting periodic unread-summary hints, which remain advisory and reconstructable.
- Changing actor mailbox triage, ownership, or reply-obligation semantics.

## Architecture

`team_actor_messages` remains the authoritative mailbox record. One
`team_runtime_delivery_receipts` row is created for each `(run_id, message_id, actor_id)` hint target.
The API and internal service persist the receipt before attempting an immediate delivery. A daemon
worker scans due receipts and retries them after restart. Receipts are deleted with their parent
mailbox message.

The state machine is:

```text
pending -> in_flight -> delivered
              |
              +------> pending
```

An `in_flight` row carries a lease. Expired leases are claimable again. Each successful claim
increments `attempt`; acknowledgement and retry updates must match that attempt so a stale worker
cannot overwrite a newer claim.

## Contracts

### Delivery Identity

- A mailbox hint ID is `team-mailbox:{run_id}:{message_id}:{actor_id}`.
- Receipt insertion is idempotent on `(run_id, message_id, actor_id)`.
- The stable ID is forwarded as the agent input message ID on transports that support message IDs
  and remains unchanged across retries.

### Delivery Meaning

- `delivered` means the current actor runtime session accepted the prompt through its input
  transport.
- It does not mean the actor read, triaged, acknowledged, or completed the mailbox message.
- `team_actor_messages.status` remains the explicit consumer-delivery/acknowledgement contract.

### Retry And Recovery

- A due `pending` receipt or expired `in_flight` receipt may be claimed by one worker.
- An unavailable runtime or input failure returns the receipt to `pending` with exponential backoff,
  capped at 60 seconds.
- One input attempt is bounded by the receipt lease, preventing a blocked runtime transport from
  stopping the entire delivery worker.
- Runtime availability is session-fenced. The receipt's mailbox run does not need to equal the
  runtime's active run because permission review may use a shared mailbox run.
- Startup recovery requires no special reset: expired leases become due through the normal query.

### Delivery Guarantee

Runtime hints are at-least-once. A crash after the runtime accepts input but before the receipt is
acknowledged may submit the same stable delivery ID again. Agent event persistence can identify the
duplicate, but external providers are not assumed to provide exactly-once execution.

## Validation Matrix

- Database initialization creates the receipt table, constraints, foreign keys, and due index.
- Worker tests cover stable IDs, transient failure, restart retry, and idempotent acknowledgement.
- Lease tests cover active-lease exclusion, expired-lease recovery, and stale-attempt fencing.
- Permission-review tests cover delivery to a current session when the message uses a shared mailbox
  run.
- Team message API tests cover stable delivery IDs in hint events and idempotent message replay.
- Cargo check, clippy, focused tests, full library tests, and the existing Bazel boundary remain the
  implementation validation gates.

## Operational Notes

- The worker polls every five seconds, claims at most 100 receipts per tick, and uses a 30-second
  lease.
- Offline actors are moved to a future retry time rather than remaining continuously due; this keeps
  old offline receipts from starving newer work in the bounded batch.
- `last_error`, `attempt`, `next_retry_at`, `lease_expires_at`, `session_id`, and `delivered_at` are
  available for database-level diagnosis. Errors are truncated to 2,048 characters.
- Periodic unread summaries use deterministic message IDs for event correlation but remain
  non-durable because the unread worker can reconstruct them from mailbox state.

## Open Risks

- Receipts for runs whose actor never returns remain pending indefinitely; retention or terminal
  abandonment policy is intentionally deferred.
- Stable IDs expose duplicate submissions for diagnosis but cannot force third-party providers to
  deduplicate execution.
- The worker belongs to the daemon background task phase and is canceled and joined before supervised
  agent process shutdown.

## Source Journals

- `docs/journal/2026-08-28-team-runtime-delivery-receipts.md`
