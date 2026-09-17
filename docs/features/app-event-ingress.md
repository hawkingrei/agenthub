# Signed App Event Intake

Status: signed HTTP intake and atomic storage implemented. Standing App-event conditions and final
slice delivery remain tracked in [active work](../todo.md#agent-loop-product-transition).

## Problem

External notifications need authenticated, replay-safe delivery without becoming a second scheduler.
Acknowledging an event separately from its wakeup or cursor would lose work across crashes.

## Scope

- Signed, bounded App notifications targeting one explicitly approved Team member.
- Atomic event identity, monotonic cursor, intake budgets, trigger, and durable receipt.
- Bounded rejection audit and safe activation source attribution.

## Non-Goals

No raw event payload, remote commands, task assignments, arbitrary future deadlines, or App-owned
execution policy. This checkpoint does not yet implement standing App-event conditions.

## Architecture

`POST /api/apps/{app_id}/events` uses App signature authentication independently of browser bearer
tokens. The daemon resolves the current [signing-key reference](app-event-configuration.md), verifies
the exact request bytes, and calls the existing loop intake inside one SQLite write transaction.
Ordinary admission owns execution, concurrency, leases, suspension, and cleanup.

## Contracts

### Wire authentication

The daemon environment reference contains exactly 32 bytes encoded with padded standard Base64
(44 characters). Provision distinct random inbound key material; outbound MCP credentials are
independent. Requests carry exactly one of each header:

| Header | Value |
| --- | --- |
| `x-agenthub-app-key-version` | Positive canonical decimal key version |
| `x-agenthub-app-timestamp` | Positive canonical decimal UTC Unix seconds |
| `x-agenthub-app-signature` | Padded standard Base64 HMAC-SHA256, exactly 32 decoded bytes |

The signed byte sequence is:

```text
agenthub.app-event.v1\n{app_id}\n{key_version}\n{timestamp}\n{exact_request_body}
```

Each displayed `\n` is one newline byte. App ID is the registered ID in the route; version and
timestamp use their canonical decimal form. There is no extra trailing newline after the body.
Delivery timestamps must be within 300 seconds of server receipt time, inclusive. Signature comparison
uses the existing HMAC library's constant-time full-length verification. Missing, duplicate,
noncanonical, stale, or invalid headers fail closed without recording a durable unauthenticated event.

### Notification and response

Bodies are limited to 8192 bytes. The only accepted JSON fields are:

```json
{
  "schema_version": 1,
  "event_id": "document-42",
  "cursor": 42,
  "team_id": "team-id",
  "actor_id": "member-id",
  "event_class": "document.changed"
}
```

The [configuration grammar](app-event-configuration.md) bounds all identifiers. Unknown fields,
including payloads and commands, are rejected. Detail retrieval belongs to approved App tools.

| Result | HTTP status |
| --- | --- |
| New durable event and trigger | 202 |
| Previously accepted identical logical event | 200 |
| Invalid bounded notification | 400 |
| Missing/invalid signature, unavailable key, or revoked App/key | 401 |
| Unauthorized class, member, binding, or route | 403 |
| Reused event ID, old cursor, or disabled target | 409 |
| Event or loop intake capacity reached | 429 |

Success returns App/event IDs, cursor, pinned manifest version, trigger/activation IDs, and a duplicate
flag. Receipts retain the validated identifier fields for logical identity comparison. These references
do not grant new execution authority. Errors do not echo request bodies, signatures,
key material, environment references, or database diagnostics.

### Atomicity and replay

Event IDs and strictly increasing positive cursors are scoped to the App across all routed targets.
Publishers serialize delivery in cursor order. Retry a transient rejection before advancing, since a
later accepted cursor prevents an earlier unseen event from being admitted.

The canonical write lock rechecks the active App and exact signing-key version, then current route,
membership, manifest version, and both authorization epochs. Revocation/rotation is checked before
returning duplicate receipts. The original decoded notification defines retry identity; insignificant
JSON formatting changes may be signed again without changing logical identity. A modified cursor,
target, or class cannot reuse an accepted event ID.

A savepoint contains budget updates, cursor advancement, ordinary loop intake, and event receipt.
Any failure rolls all of them back. A fixed rejection aggregate may then commit independently in the
outer transaction. A crash before the outer commit leaves no accepted event; a lost response after
commit is recovered by its original ID. Valid duplicates reuse the original trigger/version and update
ordinary duplicate observations without consuming new event budget.

### Budgets and audit

New logical events use durable 60-second windows starting at the first accepted event:

| Scope | Maximum accepted events per window |
| --- | --- |
| Actor, across Apps | 30 |
| Team, across Apps/members | 120 |
| App, across targets | 120 |

Clock rollback does not reset a window. Existing per-activation source and pending actor/Team limits
also apply. Suspended members retain bounded pending work; disabled or removed members cannot accept
new work. No event executes directly or bypasses the ordinary admission path.

Authenticated, structurally valid rejections aggregate into at most five rows per App: unauthorized,
ID conflict, cursor replay, capacity, and disabled. Each stores a saturating count, latest bounded event
ID, and timestamp. `GET /api/apps/{app_id}/event-audit` requires App-owner inspection authority and
returns only these aggregates. Accepted-event history retains the safe reference in its trigger;
signatures and raw bodies are never journaled.

## Validation Matrix

| Boundary | Focused proof |
| --- | --- |
| Signature | Independent known-answer vector, changed App/version/time/body/key, expiry, duplicate headers |
| Identity | Concurrent duplicate requests, modified IDs, old cursors, and exact original receipt |
| Authority | Tool-only binding, undeclared class, foreign target, route/App/key revocation and rotation |
| Transaction | Injected receipt failure leaves no cursor, budget, activation, or trigger; retry succeeds |
| Admission | Suspended pending work, disabled/capacity rollback, actor/App/Team storm caps |
| HTTP | Isolated daemon environment, signed delivery, safe responses, retry/status mapping, owner audit |

## Operational Notes

Keep clocks synchronized and persist each publisher's next cursor. Re-sign delivery retries with a
fresh timestamp and current key while preserving event identity. Do not reset cursors on key rotation
or route reapproval. Accepted receipts and loop sources remain factual history after revocation.

## Open Risks

The caps govern accepted logical work; they do not replace HTTP connection/traffic controls. Accepted
history follows the existing durable loop retention model. Standing event conditions still need the
same route authority, transaction, and replay rules before the full event slice is ready.

## Source Journals

- [Signed App intake](../journal/2026-09-18-app-event-intake.md)
