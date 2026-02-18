# Agent Actor Architecture For Local And Distributed Modes

## Background

AgentHub needs agent-to-agent collaboration without coupling runtime behavior to shell CLI commands.
Current actor mailbox semantics (`send/inbox/ack`, idempotency, pending/delivered/dead-letter) are useful and should remain stable.
What changes is execution transport and deployment mode:

- Local mode: single-node runtime with direct mailbox access.
- Distributed mode: multi-node runtime with A2A transport and centralized control.

This note defines a shared actor protocol and two separate runtime architectures.

## Goals

- Preserve one actor protocol for both local and distributed deployments.
- Keep agent mode independent from team workflow UX.
- Remove CLI from the primary coordination path.
- Support reliable agent collaboration with replayable events and deterministic retries.

## Non-Goals

- Replace existing team APIs in this phase.
- Introduce full multi-tenant policy design.
- Change current mailbox business semantics.

## Shared Protocol Contract

- Envelope fields:
  - `run_id`
  - `from_actor_id`
  - `to_actor_id`
  - `channel`
  - `transport` (`local` or `remote`)
  - `route` (optional remote route object)
  - `payload`
  - `idempotency_key`
  - `message_id`
- Core operations:
  - `actor_send`
  - `actor_inbox`
  - `actor_ack`
- Delivery model:
  - at-least-once delivery
  - idempotency-key dedupe at send boundary
  - explicit `delivered` transition via ack
- State model:
  - `pending`
  - `delivered`
  - `dead_letter`
- Retry policy guardrail (non-negotiable):
  - do not use infinite retry loops in any mode
  - only bounded retry with backoff + jitter
  - transition to `dead_letter` after max attempts
  - allow replay only via explicit operator/user requeue action

## Local Agents Architecture

- Components:
  - `AgentManager` for process/session lifecycle.
  - `ActorMailbox` storage and state transitions in local DB.
  - ACP tool adapter exposing `actor_send`, `actor_inbox`, `actor_ack`.
- Data flow:
  - Agent calls ACP actor tool.
  - Tool calls local mailbox service directly.
  - Events are persisted and replayable through existing event streams.
- Reliability constraints:
  - `session_id` guard on agent input paths.
  - deterministic idempotency key generation when missing.
  - inbox pagination by stable cursor (`id`/`created_at`).

## Distributed Agents Architecture

- Components:
  - `Coordinator`: authoritative run/task routing and actor policy.
  - `Node Agent Runtime`: local process management on each node.
  - `A2A Transport Adapter`: remote message delivery and retries.
  - `Central State Store`: source of truth for mailbox and run events.
- Data flow:
  - Sender node validates and persists message as `pending`.
  - Transport delivers to remote node endpoint.
  - Receiver validates route/auth and dedupe window.
  - Receiver persists delivery intent and returns result.
  - Sender updates state to `delivered` or schedules retry/dead-letter.
- Reliability constraints:
  - session affinity per `agent_session_id` to one node at a time.
  - retry with bounded attempts and explicit dead-letter transition.
  - no automatic retry after `dead_letter`; re-delivery must be explicit.
  - replay-safe receiver dedupe by `(run_id, message_id|idempotency_key)`.

## Security Model

- Local mode:
  - existing auth/session checks
  - safe-path and tool permission constraints remain unchanged
- Distributed mode:
  - mTLS between nodes
  - scoped actor capability tokens for `send/inbox/ack`
  - strict route validation and anti-replay time window

## Observability

- Required counters:
  - actor send total
  - dedupe hits
  - ack latency
  - retry count
  - dead-letter count
- Required logs:
  - message lifecycle transitions with actor/run identifiers
  - remote delivery errors with categorized reason
- Required traces:
  - end-to-end span from `actor_send` to `actor_ack`

## Rollout Plan

- Phase 1:
  - Introduce ACP-native actor mailbox tools.
  - Keep CLI as optional debug fallback only.
- Phase 2:
  - Add transport abstraction (`local` + `remote`) behind one service interface.
  - Keep local mode default.
- Phase 3:
  - Enable distributed runtime with coordinator + node runtime.
  - Run hybrid validation (local and distributed side-by-side).
- Phase 4:
  - Promote distributed mode for selected runs and keep local fallback.

## Testing Strategy

- Local mode tests:
  - send/inbox/ack idempotency and ordering
  - session mismatch guard behavior
  - replay consistency after restart
- Distributed mode tests:
  - remote retry and dead-letter transitions
  - duplicate delivery and dedupe correctness
  - node outage and recovery behavior
- End-to-end tests:
  - multi-agent delegation chain with evidence-return payloads
  - mixed local/remote actor routing in one run

## Open Questions

- Should global ordering use DB auto-increment only, or a dedicated sequence service for distributed mode?
- Which route schema is mandatory for remote actor delivery (`endpoint`, `cluster`, `auth_ref`)?
- Should receiver-side ack be immediate-on-persist or post-processing confirmed?
