# Team Conversation Event Bus Specification

## Problem

Team collaboration currently relies on mailbox and run-scoped APIs for deterministic execution.
For human-facing daily communication and real-time chat UX, direct mailbox-only interaction is too
heavy:

- users should not need to know `run_id`;
- chat flow should support `@member` routing naturally;
- leader/worker should see a shared live timeline without weakening mailbox guarantees.

We need a conversation-first event-bus contract that preserves execution correctness.

## Scope

- Conversation event-bus role and boundaries.
- `conversation -> task -> run` identity mapping and lifecycle.
- Input normalization rules (`@mention`, auto-filled actor identity, correlation chain).
- Routing split between event bus and mailbox.
- P2P (`main` + `node`) compatibility baseline.

## Non-Goals

- Replacing mailbox/actor state machine semantics.
- Defining provider-specific MCP/SDK details.
- Final broker product selection (Kafka/Pulsar/NATS) in this document.

## Architecture

### 1) Layered Message Plane

- Command plane (authoritative execution):
  - mailbox + actor (`send/inbox/ack`, idempotency, dead-letter, run partitioning).
- Communication plane (human-facing timeline):
  - event bus as chat/event carrier for real-time fan-out and replay.

### 2) Source Of Truth Rule

- `main` node database is the source of truth for:
  - conversation messages
  - task records
  - mailbox state
  - run events
- Event bus is delivery infrastructure, not authoritative storage.

### 3) Outbox Delivery Rule

- Persist-first, then publish:
  1. write DB records (`conversation`/`run_event`/`mailbox` as needed);
  2. write `event_outbox`;
  3. relay publishes to event bus;
  4. consumer fan-out to chat page/SSE/WebSocket.
- No direct publish-before-persist path.

### 4) Main And Node Topology

- `main`:
  - assigns global event order and persists canonical records.
- `node`:
  - subscribes event partitions needed for execution/runtime display;
  - sends execution outcomes back to `main`, which finalizes authoritative state.

## Contracts

### 1) Identity And Lifecycle Mapping

Canonical mapping:

- `conversation_id` (UUIDv7): human-facing long-lived context.
- `task_id` (UUIDv7): leader-defined internal work item under a conversation.
- `run_id` (UUIDv7): one concrete execution instance generated when execution starts.

Lifecycle relation:

- one conversation can contain multiple tasks;
- one task can compile into multiple runs (retry/restart/replan);
- before execution, `run_id` may be absent.

Required invariant:

- user-facing APIs should not require user-supplied `run_id`.

### 2) Input Normalization Contract

For human/agent chat input in conversation lane:

- client payload may omit:
  - `from_actor_id`
  - `run_id`
- gateway must enrich:
  - `from_actor_id`: derived from authenticated human session or agent session identity;
  - `run_id`: optional; bind active run if exists, otherwise keep null (conversation-scoped);
  - `to_actor_ids`: derived from channel fan-out rules, not from `@member_id` mentions;
  - `mention_actor_ids`: derived from `@member_id` mentions and preserved for receivers;
  - `conversation_id`: required.

Routing by mention:

- with or without `@member_id`: group chat still uses team broadcast scope.
- `@member_id` is preserved as mention metadata for receivers and UI.

### 3) Event Envelope Contract

Minimum event fields for conversation bus:

- `event_id` (global, monotonic in main DB)
- `conversation_id`
- `from_actor_id`
- `event_type`
- `payload`
- `ts`

Optional linkage fields:

- `task_id`
- `run_id`
- `to_actor_ids`
- `correlation_id`

### 4) Correlation ID Contract

Definition:

- `correlation_id` links a chain of related messages/events across conversation, mailbox, and run events.

Usage:

- one request-intent chain shares one `correlation_id`, for example:
  - leader assignment -> worker progress -> worker result -> leader synthesis.

Constraints:

- `correlation_id` is not `message_id`;
- keep stable across retries of the same intent;
- generate as UUIDv7 when a new chain starts.

### 5) Transport Split Contract

Communication-only message types (event bus primary, no mailbox required):

- `chat_message`
- `status_note`
- `decision_note`
- `checkpoint`

Execution-command message types (mailbox required, may mirror to event bus for visibility):

- `assignment`
- `approval_request`
- `approval_result`
- `step_action`
- `execution_result`

Rule:

- execution semantics must not depend only on event-bus consumer ack.
- mailbox ack remains the execution completion signal.

### 6) Ordering And Idempotency Contract

- global event order is assigned by `main` (`event_id` sequence).
- event bus delivery can be at-least-once.
- consumers/UI must dedupe by `event_id`.
- mailbox idempotency remains keyed by mailbox constraints (`run_id` + actor semantics + idempotency key).

## Validation Matrix

1. Conversation-first input
- user sends message without `run_id`/`from_actor_id`, gateway fills required fields correctly.
- `@member` is preserved as mention metadata while channel mailbox fan-out stays broadcast.
- current baseline implementation: task conversation send API accepts omitted `from_actor_id`
  and defaults to authenticated canonical user actor.
- current baseline implementation: task conversation payload ensures `correlation_id`
  (generated when missing) so chat and mailbox forwarding can share one intent-chain id.
- current baseline implementation: frontend task chat sender no longer computes mailbox routing;
  backend handles mention extraction and mailbox fan-out for active runs.

2. Mapping correctness
- `conversation_id -> task_id -> run_id` relation is queryable and replayable.
- task exists before run in planning-only phase.

3. Transport split
- communication events appear in chat timeline through event bus.
- execution-command path still requires mailbox and preserves ack evidence.

4. Reliability
- outbox relay restart does not lose persisted events.
- duplicate bus deliveries are deduped by `event_id`.

5. P2P readiness
- `node` can subscribe conversation/run partitions while `main` stays authoritative.

## Operational Notes

- Keep mailbox as execution truth and event bus as communication carrier.
- Prefer UUIDv7 for `conversation_id`/`task_id`/`run_id`/`correlation_id`.
- Keep user API minimal: user should provide goal + message + optional `@mention`, not internal execution IDs.
- For chat page, use `last_event_id` cursor resume and API backfill for gap recovery.

## Open Risks

- Event type misclassification can leak execution commands into communication-only channel.
- High-volume broadcast traffic may need partition and retention tuning.
- Cross-node clock skew should not be used for ordering; rely on main-assigned event IDs.

## Source Journals

- `docs/journal/2026-03-05-team-mcp-enforcement-lessons-from-slock.md`
- `docs/journal/2026-03-05-main-node-terminology-and-doc-pruning.md`
- `docs/journal/2026-02-22-team-task-routing-user-actor-semantics.md`
