# Agent Reminders

## Problem

Long-lived agents need to schedule a follow-up without holding a turn open or relying on a browser.
Standalone agents and Team members share the same one-shot reminder service.

## Scope

- Self-service create, list, and cancel commands for managed agents.
- Persistent scheduling, fenced dispatch attempts, bounded retries, and source correlation.
- Local and remote ACP delivery after active turns finish.
- Inspectable submission status and retry information.

## Non-Goals

Recurring schedules, snooze, automatic process restart, canonical Team task management, and proof of
agent execution are outside this contract. A reminder does not create new human intent or permissions.

## Architecture

SQLite `agent_time_triggers` is the scheduling authority. The daemon polls every two seconds and
claims at most 32 due rows atomically with `UPDATE ... RETURNING`. Each attempt increments a fencing
counter and holds a 90-second lease. Up to eight submissions run concurrently, each with a ten-second
submission timeout. Failed attempts back off for 5, 10, 20, 40, 80, 160, then 300 seconds, capped at 300.
The original `fire_at` is preserved; `next_attempt_at` controls retry eligibility.

Scheduled rows and expired dispatch leases are eligible for claim. Startup only releases expired or
legacy lease-less dispatches; it cannot steal an active lease from another daemon. Completion and
requeue updates require the current attempt, an unexpired lease, and `dispatching` status.

## Contracts

### Agent entry points

```bash
agenthub actor time-trigger-set --delay-seconds 120 --message "Check task progress" --source-ref "task:123" --json
agenthub actor time-trigger-list --json
agenthub actor time-trigger-cancel --trigger-id <trigger-id> --json
```

`--source-ref` is optional correlation text, limited to 1,024 bytes. Messages are limited to 16,384
bytes and schedules to 1 through 2,592,000 seconds. The CLI sends a relative delay to the server,
which computes the deadline using its own clock. The legacy absolute `fire_at` RPC remains accepted;
callers must not send both an absolute deadline and a relative delay.

The executor supplies `AGENTHUB_ACTOR_AGENT_ID` in both standalone and Team sessions. Reminder commands
prefer this identity and reject an explicit different target. Standalone processes do not inherit
Team actor/run/channel variables or an ancestor's internal gRPC target/token. ACP sessions receive a
bounded, provider-neutral reminder instruction when internal gRPC is enabled; this does not install
Team role skills for standalone agents. Raw stdin runtimes can use the same CLI but have no ACP turn
boundary or injected ACP instructions.

The existing local runtime connector loads the daemon configuration and obtains a fresh, short-lived
Worker token with only `agent:time_trigger:manage` for these commands. Local agents therefore need the
same readable configuration and internal gRPC connectivity as other local actor commands. This is
within the existing trusted-host model, not a new subprocess sandbox. Explicit remote runtime
credentials retain their configured permission and lifetime limits. Server authorization binds
Worker tokens to their own actor for create, list, and cancel; a reminder-only token grants no Team
read/write or general agent-management permissions.

### Source and scope

Creation captures `session_id`, `team_id`, and `run_id` from the target runtime together with the
optional source reference. Remote targets supply their snapshot through `GetAgentRecord`.
`scope_bound` distinguishes new snapshots from legacy records with no provenance. New reminders
must match the target's current Team and run, including standalone-to-Team transitions. A session
restart within the same scope is allowed. A scope mismatch is retained as a retry error until the
reminder is canceled or its original scope becomes available again.

The source reference is not dereferenced and does not establish task ownership or permission.
The injected text directs the agent to check current task state before acting. The persisted ACP
input event retains its compatible `user_message` shape and adds `origin.kind = "reminder"` and
source metadata; it is not a new human message for authorization purposes.

### Submission and cancellation

| Status | Meaning |
| --- | --- |
| `scheduled` | Waiting for its deadline or retry eligibility. |
| `dispatching` | A leased submission attempt is in progress. |
| `fired` | Input was accepted into the runtime command channel, or written to raw stdin. |
| `canceled` | Future claims are disabled; in-flight completion cannot overwrite cancellation. |

The web inspector displays `fired` as `submitted` and explicitly states that execution is unconfirmed.
It refreshes serially every ten seconds while mounted, discarding stale responses on agent changes.
Retry time, attempt count, source reference, and the latest error remain visible.

ACP reminders use a deferred command, even for providers that allow concurrent human prompts. They
wait behind active turns and permission waits. Ordinary input retains its existing steering policy.
Stopped processes are never started by reminders; submission failure leaves the reminder retryable.
Raw stdin delivery retains its existing write-and-flush semantics without a turn-idle guarantee.

Cancellation prevents claims and fences state changes, but cannot retract input whose submission
has already begun or which is already in the runtime queue. `fired_at` is a submission timestamp,
not an execution timestamp. A stable `time-trigger:<id>` message identity is retained across retries.
A crash after submission but before recording `fired` may duplicate input. A crash after recording
`fired` but before the queued turn executes may lose that execution. There is no exactly-once or
execution-acknowledgment claim; inspect agent output for actual execution evidence.

### Remote compatibility and migration

Remote reminders use `SendAgentReminder`, a separate internal RPC. An older peer returns
`UNIMPLEMENTED` instead of silently treating a reminder as ordinary concurrent input. Older peers
without source snapshots cannot accept newly created remotely scoped reminders. Both failures are
inspectable and require upgrading the peer; there is no unsafe fallback.

The additive SQLite migration preserves existing rows and adds `attempt`, `next_attempt_at`,
`lease_expires_at`, and `source_json`, plus a retry index. Legacy pending rows remain deliverable
without invented scope metadata. Existing status and timestamp fields remain wire compatible.

## Validation Matrix

| Boundary | Regression evidence |
| --- | --- |
| Legacy storage | Repeatable migration preserves dispatching rows and timestamps. |
| Claims | Concurrent claimers cannot claim the same live lease. |
| Cancellation | Late success and failure cannot overwrite canceled state. |
| Recovery | Live leases survive startup; expired attempts are fenced after reclaim. |
| Retries | Backoff preserves deadlines and allows other due work through. |
| Lifecycle | A stopped agent is not started; session restarts preserve compatible scope. |
| Scope | Team/run changes reject delivery; reminder-only tokens cannot address other agents. |
| ACP | Reminders defer under concurrent-prompt policies and permission waits. |
| CLI | Standalone identity and source reference resolve without Team context. |
| Web | Submission wording, retry provenance, serial refresh, and stale-response suppression. |

## Operational Notes

Agent self-service requires `[internal_grpc] enabled = true`, which is disabled by default. This
change preserves that configuration gate. HTTP scheduling and inspection do not require the agent
CLI connector; each CLI uses its configured control endpoint and that endpoint owns its records.

Monitor `last_error`, attempt count, and retry eligibility when a reminder remains pending. Cancel
obsolete reminders instead of waiting for them to restart an agent or revive an old Team run.
Upgrade both the control plane and remote nodes for the new deferred-delivery contract.

## Open Risks

Execution acknowledgments, finite retry policies for permanently obsolete scopes, creation
idempotency, quotas, and periodic scheduling remain future work. Legacy reminders cannot enforce a
source scope that was never stored. This implementation requires runtime smoke verification after
deployment; unit tests and preview rendering do not prove live provider behavior.

## Source Journals

- [Reminder implementation checkpoint](../journal/2026-09-06-agent-reminders.md)
