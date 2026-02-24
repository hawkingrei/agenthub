# Actor Foundation Specification

## Problem

Actor is the fundamental coordination primitive for AgentHub Teams, but it is easy to confuse it
with agent process, member role, or UI-level task objects. Without a stable actor contract,
Team behavior can drift across runtime, API, and tool boundaries.

## Scope

- Define Actor as the base protocol for agent-to-agent and human-to-agent coordination.
- Clarify Actor relationship with Team concepts (`member`, `agent`, `run`, `step`, `conversation`).
- Define Actor identity, partitioning, delivery semantics, and failure/retry model.
- Define local/distributed evolution path without changing actor semantic contracts.

## Non-Goals

- Replacing Team run/step orchestration model.
- Defining model-prompt wording.
- Full multi-tenant authorization matrix design.

## Architecture

### 1) Actor As The Coordination Substrate

Actor is the message-transport and delivery contract under Team runtime.
Team planning/execution sits above Actor:

- Team layer decides **what** to do (`conversation -> run -> step`).
- Actor layer guarantees **how** messages move and are acknowledged (`send -> inbox -> ack`).

### 2) Actor And Team Object Mapping

- `member`: stable logical identity in Team spec.
- `agent`: runtime process bound to a member.
- `actor_id`: mailbox sender/receiver identity used by actor protocol.
- `run_id`: mailbox partition boundary for one execution context.

Practical mapping:

- one member/agent can use one actor identity in a run;
- actor identities are isolated by `run_id` to keep replay deterministic.

### 3) Core Actor Operations

- `actor_send`
  - enqueue message with idempotency semantics.
- `actor_inbox`
  - fetch pending messages by actor identity and run partition.
- `actor_ack`
  - mark message delivered and attach optional result/evidence.

### 4) Delivery State Model

- `pending`
- `delivered`
- `dead_letter`

Transition rules:

- `pending -> delivered` via valid ack;
- `pending -> dead_letter` when retries exhausted or non-retryable failure;
- `dead_letter` is terminal unless explicit requeue action;
- ack must be idempotent for `(run_id, actor_id, message_id)`.

### 5) Local And Distributed Runtime Modes

Actor semantic contract remains stable in both modes:

- Local mode:
  - mailbox and state transition in local DB;
  - runtime tools call local actor service directly.
- Distributed mode:
  - transport adapter delivers across nodes;
  - central coordinator/state keeps same actor operation semantics.

## Contracts

### 1) Identity Contract

- Canonical field names: `actor_id`, `from_actor_id`, `to_actor_id`.
- Tool boundary aliases (`agent_id`) are additive compatibility aliases only.
- `run_id` is required partition key and cannot be dropped.

### 2) Message Envelope Contract

Minimum envelope fields:

- `run_id`
- `from_actor_id`
- `to_actor_id`
- `channel`
- `payload`
- `idempotency_key` (optional input, required effective behavior)
- `message_id` (server-assigned)

Identity-kind projection (for UX/policy):

- `from_actor_kind`: `human|agent`
- `to_actor_kind`: `human|agent`

### 3) Reliability Contract

- Delivery model is at-least-once + idempotent send/ack handling.
- Retry must be bounded with backoff+jitter.
- No infinite retry loops.
- Dead-letter replay must be explicit operator action.

### 4) Team Integration Contract

- Team leader/worker coordination should route through Actor mailbox in run scope.
- Team run/step state updates must not bypass actor delivery evidence for cross-actor communication.
- Human actor is a first-class actor kind and should be represented explicitly.

### 5) Observability Contract

Required visibility:

- lifecycle logs for `send/inbox/ack/retry/dead_letter`;
- counters for send total, dedupe hit, retry count, dead-letter count;
- run/actor scoped traces for message path debugging.

## Validation Matrix

- `cargo test teams_router_http_contract -- --nocapture`
- `cargo test internal::service::tests -- --nocapture`
- `cargo test -p agenthub-team-actor`

## Operational Notes

- Keep actor semantics stable even when transport evolves (local -> distributed).
- Keep operator-facing docs explicit: `run_id` partitions mailbox, `actor_id` identifies sender/receiver.
- Prefer compatibility aliasing (`agent_id`) over field renaming in storage/API internals.

## Open Risks

- Human/agent identity-kind is currently convention-driven and may need stricter typed enforcement later.
- Cross-node ordering and replay policy in distributed mode still requires additional hardening.

## Source Journals

- `docs/journal/2026-02-18-agent-actor-local-distributed-architecture.md`
- `docs/journal/2026-02-24-actor-agent-id-alias.md`
- `docs/journal/2026-02-18-acp-actor-mailbox-native-tools.md`
