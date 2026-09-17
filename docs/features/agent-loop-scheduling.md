# Agent Loop Scheduling

## Problem

An offline actor needs durable future work without keeping a provider session alive. Polling the
current task state can miss a dependency that changed and changed back between scans. Replaying
every missed timer interval can overload admission after a restart.

## Scope

- One-shot due times, recurring timers, task-status conditions, thread-reply watches, and signed App-event conditions.
- Actor and owner registration, bounded inspection, revocation, and ordinary activation admission.
- Recovery after process loss and cancellation of work derived from obsolete registrations.

## Non-Goals

- Legacy process reminders, arbitrary predicates, cron expressions, or a second scheduling engine.
- Task assignment, task acceptance, roster changes, or approval authority through scheduling.
- Arbitrary external payload subscriptions; [signed App notifications](app-event-ingress.md) have explicit routing authority.

## Architecture

`loop_registrations` stores immutable scheduling intent, origin references, an observation cursor,
and at most one pending firing. `loop_registration_firings` links accepted cursor ranges to the
ordinary durable trigger and activation records. Message bodies remain in canonical message stores.

Registration and its initial dependency read hold the same SQLite write transaction. Every canonical
runtime task-status writer and threaded-message insert updates matching registrations inside its
own transaction. This includes run-to-task status synchronization. No timer poll is responsible for
remembering a transient condition. Human-visible mailbox reply copies contain no thread-root metadata
and are not a second thread event source.

The daemon reconciles at most 32 due registrations per tick before ordinary admission. Each firing,
its intake receipt, and the registration advance commit together. A savepoint rolls back partial
intake when capacity or disabled policy defers acceptance. Pending observations remain durable and
retry after five seconds. Per-actor and per-Team standing limits bound all active registrations,
including one-shot waits; the tightest enabled/suspended Team policy applies.

## Contracts

### Intent and identity

A request contains `source_key`, `schedule`, and optional `work_task_id`. Target member and Team scope
come from the authenticated route or signed executor context. Scheduling actor, activation, and user
attribution cannot be supplied in the request document. Business keys are namespaced by creator and
target. A retry from a later activation of the same creator returns the original registration and
provenance; changing the requested work is an idempotency conflict. A completed or revoked key does
not recreate work. Use a new business key for a new registration.

All schedule timestamps are nonnegative Unix seconds. Supported schedules are:

| Kind | Fields | Firing condition |
| --- | --- | --- |
| `due` | `due_at` | Once, at or after the deadline |
| `recurring` | `first_at`, `interval_seconds` (1–86400) | One catch-up firing for all overdue intervals, then the next future deadline |
| `task_status` | `task_id`, nonempty unique `statuses`, `repeat` | Initially satisfied, or a false-to-true condition edge |
| `thread_reply` | `root_message_id`, `after_message_id`, `repeat` | Later canonical replies in that thread, excluding the target actor's own replies |
| `app_event` | `app_id`, `event_class`, nonnegative `after_cursor`, `repeat` | Accepted notifications for that exact target and approved class, including initial history catch-up |

Task statuses use the canonical task enum. Rewriting notes or an unchanged matching status does not
create another edge. A pending match survives subsequent nonmatching states. Repeated observations
before acceptance coalesce: the firing records its first cursor and latest observed cursor, rather
than claiming an independent activation for every intermediate state. Recurrence catch-up uses
arithmetic rather than iterating missed intervals. A deadline beyond representable time ends the
recurrence after its final valid firing.

A thread cursor must identify the root or an existing reply in that exact conversation and thread.
Registration examines replies after that cursor under the same write lock. The firing references an
exact canonical reply, recoverable through `loop-source`; recent-message windows and delivery copies
are not prerequisites. Further replies coalesce while acceptance is deferred.

App event watches pin current route authority and use indexed receipt cursors. They observe accepted
notifications in the same transaction and retain original event attribution in each firing source.
Authority changes revoke idle and completed watches; delivery retries never refire them. The complete
scope, route-reapproval, and lifetime contract is [standing App conditions](app-event-ingress.md#standing-conditions).

### Admission and lifetime

Firings use the normal source, pending-work, rate, startup, reservation, and no-progress budgets.
Suspension retains registrations and accepted work while pausing admission. Disablement rejects new
registrations and retains existing pending observations until re-enabled. Registering future work or
observing a dependency never starts a provider directly and does not reset no-progress counters.

`work_task_id` binds registration lifetime to the work being performed; it is separate from the
watched dependency task. Completing or canceling that work revokes its registrations and outstanding
sources. Deleting a task/channel revokes registrations that depend on its task or thread. Cancellation
of the creating activation revokes its registrations; normal structured finish preserves them.

Revocation is idempotent and retains history. It clears the pending observation and revokes all sources
accepted from that registration. An activation with no independent remaining source is canceled,
including a currently running activation. That cancellation invalidates executor calls immediately;
its execution reservation remains until normal verified process cleanup. Independently coalesced
work survives. Registrations created by exclusively dependent canceled activations are also revoked.

Active registrations block scope changes. Any retained registration history keeps the actor's original
Team identity, like activation history; copying a Card creates a distinct identity instead of moving
scheduling authority or history.

### Actor and owner surfaces

Actor commands require live activation credentials:

```bash
agenthub actor loop-schedule --request-file request.json
agenthub actor loop-schedule --member-id worker-id --request-file request.json
agenthub actor loop-schedules --member-id worker-id --limit 64
agenthub actor loop-schedule-show --registration-id registration-id --limit 64
agenthub actor loop-schedule-revoke --registration-id registration-id
```

The default target/list member is the signed actor. `loop-schedule` help documents the request forms.
Register future work before `loop-finish`, then exit. List pages use `next_cursor` with
`--after-registration-id`; detail pages use `next_firing_cursor` with `--after-firing-cursor`. Both
page sizes are 1–256, default 64. Completed and revoked records remain inspectable. Detail reads use
one database snapshot for the registration and its firing history.

The corresponding RPCs are `RegisterLoopSchedule`, `ListLoopSchedules`, `GetLoopSchedule`, and
`RevokeLoopSchedule`. Reads require `team:read`; registration and revocation require `loop:activate`.
The target member, creator, or current canonical coordinator may revoke. Token role labels alone
cannot grant coordinator authority. Reads remain within the current Team.

Owner HTTP routes use `/api/teams/{team_id}/members/{member_id}/loop/schedules`:

- `POST` registers the request document; `GET` lists with `after_registration_id` and `limit`.
- `GET /{registration_id}` reads detail with `after_firing_cursor` and `limit`.
- `DELETE /{registration_id}` revokes; writes require owner authority and `runtime:operate`.
- Reads require Team access and `runtime:inspect`. A registration must match both path scopes.

Request JSON rejects unknown fields. Actor documents are limited to 16 KiB. Capacity rejection uses
HTTP 429 or gRPC `resource_exhausted`. Scheduling does not grant task or roster authority.

## Validation Matrix

| Boundary | Required evidence |
| --- | --- |
| Persistence | File reopen, stable firing identity, duplicate reconciliation, bounded catch-up |
| Dependency | Changes before/during/after registration, transient match and reversion, unchanged writes |
| Thread | Initial cursor recovery, self-reply exclusion, coalescing, exact source and scope validation |
| Budgets | Standing fan-out, 32-row reconciliation, capacity/disabled retention, suspension, no-progress stop |
| Revocation | Fire race, pending/running cancellation, independent sources, retained reservation, parent/task deletion |
| Surfaces | Signed provenance, creator/target/coordinator/owner authorization, stale credentials, bounded paging |
| Integration | Canonical task and run-status writers, canonical thread inserts, CLI parsing and help discovery |

## Operational Notes

A pending observation is not yet an accepted trigger. Inspect `pending_cursor`, `pending_due_at`, and
`next_check_at` alongside the target's execution policy. Inspect firing receipts for accepted work and
activation history for admission or cleanup state. Do not recreate registrations to bypass budgets.

## Open Risks

- Future canonical dependency writers must call the observation hook in their write transaction.
- Retained firing history grows with accepted work; lifecycle history retention remains a separate
  administrative contract. Active intent and per-tick processing are bounded now.
- Provider and signed-ingress contracts retain their own integration evidence alongside scheduler tests.

## Source Journals

- [Bounded follow-ups and standing triggers](../journal/2026-09-15-agent-loop-scheduling.md).
- [Durable work events](../journal/2026-09-15-agent-loop-work-events.md).
- [Signed App intake and conditions](../journal/2026-09-18-app-event-intake.md).
