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

## API Contract Draft

### `actor_send`

- Request:
  - `run_id` (string, required)
  - `from_actor_id` (string, required)
  - `to_actor_id` (string, required)
  - `channel` (string, required)
  - `payload` (json, required)
  - `idempotency_key` (string, optional; server derives deterministic key when absent)
  - `transport` (`local|remote`, optional; default `local`)
  - `route` (object, optional for `remote`)
- Response:
  - `message_id` (string)
  - `state` (`pending|delivered`)
  - `deduped` (bool)
  - `created_at` (rfc3339 timestamp)

### `actor_inbox`

- Request:
  - `run_id` (string, required)
  - `actor_id` (string, required)
  - `cursor` (string, optional)
  - `limit` (int, optional, default `50`, max `200`)
  - `states` (array of state filter, optional, default `pending`)
- Response:
  - `messages` (array of message records)
  - `next_cursor` (string, optional)

### `actor_ack`

- Request:
  - `run_id` (string, required)
  - `actor_id` (string, required)
  - `message_id` (string, required)
  - `ack_token` (string, optional when optimistic-ack is disabled)
  - `result` (json, optional, for evidence/status payload)
- Response:
  - `message_id` (string)
  - `state` (`delivered`)
  - `acked_at` (rfc3339 timestamp)

### Error Contract

- `400 bad_request`: invalid payload/field format.
- `401 unauthorized`: auth/session invalid.
- `403 forbidden`: actor capability missing.
- `404 not_found`: actor or run not found.
- `409 conflict`: state conflict (stale session/state mismatch/duplicate ack).
- `410 gone`: message expired and no longer ackable.
- `422 unprocessable_entity`: remote route invalid.
- `429 too_many_requests`: mailbox throttled.
- `5xx`: infrastructure/runtime error.

## State Machine

- Transitions:
  - `pending -> delivered` by valid `actor_ack`.
  - `pending -> pending` by retry scheduler on transient send failure.
  - `pending -> dead_letter` on max-attempt reached or non-retryable failure.
  - `dead_letter -> pending` only through explicit requeue operation.
  - `delivered` is terminal.
- Invariants:
  - `actor_ack` must be idempotent by `(run_id, actor_id, message_id)`.
  - `dead_letter` must never auto-transition to `pending`.
  - duplicate `actor_send` with same idempotency key must return original `message_id`.

## Retry And Backoff Parameters

- Suggested config surface:
  - `actor_retry.max_attempts`
  - `actor_retry.base_delay_ms`
  - `actor_retry.max_delay_ms`
  - `actor_retry.jitter_ratio`
  - `actor_retry.dead_letter_ttl_hours`
- Backoff recommendation:
  - `delay = min(max_delay_ms, base_delay_ms * 2^attempt) + jitter`
  - jitter range should be symmetric to avoid coordinated retry spikes.

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

## Implementation Snapshot (2026-02-18)

- Added shared actor mailbox contract types in `crates/agenthub-team-actor`:
  - `ActorSendRequest/Response`
  - `ActorInboxRequest/Response`
  - `ActorAckRequest/Response`
  - `ActorServiceErrorCode`, `ActorServiceError`
  - `ActorMailboxService` trait
- Added `TeamActorMailboxService` adapter in `src/team/manager/mailbox.rs`:
  - maps contract calls to existing `TeamManager` mailbox operations
  - keeps idempotency conflict and row-not-found mapping explicit
  - keeps inbox limit compatibility at `1..1000` for current runtime behavior
- Wired internal gRPC mailbox paths to the new service skeleton:
  - `TeamInternalControl.send_actor_message`
  - `TeamInternalControl.list_actor_inbox`
  - `TeamInternalControl.ack_actor_message`
  - kept response payload shape unchanged for existing callers
- Wired Teams HTTP mailbox paths to the same service skeleton:
  - `POST /api/teams/runs/:run_id/messages/send`
  - `GET /api/teams/runs/:run_id/messages/inbox`
  - `POST /api/teams/runs/:run_id/messages/:message_id/ack`
  - kept API response payload shape unchanged for existing callers
- Added ACP-native actor mailbox tool wiring for actor sessions:
  - ACP runtime auto-injects stdio MCP server `agenthub-actor-mailbox`
  - server command uses AgentHub binary `actor-mcp` subcommand with actor context args
  - actor runtime skill now guides `actor_inbox` / `actor_ack` / `actor_send` native tool usage instead of CLI commands

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
