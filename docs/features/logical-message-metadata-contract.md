# Logical Message Metadata Contract

## Problem

AgentHub already carries message identity and delivery metadata across multiple layers:

- canonical conversation rows on `main`;
- actor mailbox rows on `main`;
- remote relay envelopes between `main` and `node`;
- node-local replica/cache rows;
- search/archive projections.

Today these layers already share useful fields such as `run_id`, `conversation_id`,
`authority_message_id`, `correlation_id`, `broadcast_id`, and `idempotency_key`, but the system
does not yet define one canonical ownership contract for them. Without that contract, future work
on distributed replay, search, gossip-driven node membership, and multi-tenant isolation can drift
into incompatible metadata semantics.

## Scope

- define the canonical metadata fields for one logical message;
- define which fields are authority-layer fields vs delivery-layer fields;
- define which surfaces on `main` are authority rows and which surfaces on `node` are cache or
  projection rows;
- define the minimum metadata that every human-visible or searchable message projection must carry.

## Non-Goals

- redesigning Team channel/thread UX;
- replacing mailbox delivery semantics;
- introducing a full cross-node total ordering protocol in this step;
- defining the final physical schema for every future archive or search backend.

## Architecture

AgentHub should model message metadata in three layers.

### 1) Authority Layer

This layer defines the logical message and execution truth. Authority-layer fields must be owned by
`main` and must not be redefined by node-local caches or projections.

Fields:

- `group_id`
- `run_id`
- `conversation_id`
- `authority_message_id`
- `correlation_id`

### 2) Delivery Layer

This layer defines one delivery or fan-out attempt. Delivery metadata may be derived, retried, or
replayed, but it must not redefine the logical message identity.

Fields:

- `broadcast_id`
- `source_node_id`
- `target_node_id`
- `idempotency_key`

### 3) Projection Layer

This layer contains cache, replica, search, or UI-facing materialized views. Projection rows may be
discarded or rebuilt, but they must always preserve a path back to authority-layer identity.

Minimum projection fields:

- `run_id`
- `conversation_id`
- `authority_message_id`
- `correlation_id`

For cross-node projections, also preserve:

- `broadcast_id` when the logical message arrived via channel fan-out or other group delivery;
- `source_node_id`
- `target_node_id`
- `idempotency_key` when the projection needs retry/debug traceability.

## Contracts

### 1) Main Authority Contract

`main` is the authority node for distributed message truth.

It owns the authority rows for:

- canonical human-visible conversation messages;
- mailbox delivery state (`pending`, `delivered`, `dead_letter`);
- channel/thread authority identity;
- run/event delivery evidence.

Node-local stores must not independently redefine these rows.

### 2) Node Cache Contract

Non-`main` nodes keep only relevant message caches and projections:

- mailbox rows addressed to local actors;
- channel/thread context needed for local execution;
- search/archive projections built from authority-linked message metadata.

Node-local rows are disposable caches. If the node loses them, it must be able to rebuild or
reconcile them from `main` using authority references.

### 3) Logical Identity Contract

- `authority_message_id` is the canonical logical message identity.
- `correlation_id` is the canonical intent-lineage identity.
- `conversation_id` is the canonical conversation-container identity.
- `run_id` is the canonical execution-partition identity.

These fields serve different purposes and must not be conflated:

- `authority_message_id` identifies one message;
- `correlation_id` identifies one intent chain that may span multiple messages;
- `conversation_id` identifies one human-visible conversation container;
- `run_id` identifies one execution scope.

### 4) Delivery Metadata Contract

- `broadcast_id` identifies one fan-out or distributed delivery instance.
- `source_node_id` and `target_node_id` identify the current transport hop.
- `idempotency_key` identifies a stable send/fan-out/retry dedupe intent.

These fields are delivery metadata, not logical identity fields.

### 5) Authority Rows vs Replica/Projection Rows

Authority rows:

- `team_conversation_messages` for canonical human-visible message content;
- `team_actor_messages` for canonical mailbox delivery state.

Replica / projection rows:

- `team_channel_message_replicas`;
- node-local mailbox caches;
- node-local conversation/thread caches;
- archive/search documents such as LanceDB message rows.

Replica/projection rows must carry enough references to reconcile against authority rows, but they
must not be treated as the source of truth when data diverges.

### 6) Current Surface Ownership Matrix

| Surface | Role | Required authority references | Notes |
| --- | --- | --- | --- |
| `team_conversation_messages` | authority row | `conversation_id`, `authority_message_id`, `correlation_id` | canonical human-visible message content |
| `team_actor_messages` | authority row | `run_id`, sender/recipient actor ids, effective `idempotency_key` | canonical delivery state |
| `team_channel_message_replicas` | replica row | `run_id`, `conversation_id`, `authority_message_id`, `correlation_id` | node-relevant channel cache only |
| remote relay route metadata | delivery metadata | `source_node_id`, `target_node_id`, optional `broadcast_id`, optional `correlation_id`, effective `idempotency_key` | transport/debug only |
| archive/search document | projection row | `run_id`, `conversation_id`, `authority_message_id`, `correlation_id` | must point back to `main` authority |

## Validation Matrix

- `cargo test internal_grpc_mailbox_send_persists_channel_replica_history -- --nocapture`
- `cargo test internal_grpc_mailbox_send_rejects_channel_replica_payload_without_correlation_id -- --nocapture`
- `cargo test remote_actor_messages_relay_success_marks_message_delivered -- --nocapture`
- `cargo test bidirectional_actor_grpc_pipeline_relays_seeded_messages_between_in_process_states -- --nocapture`

## Operational Notes

- Prefer `main` authority plus node-local relevant-message cache over multi-node message truth
  synchronization.
- Node-local archive/search systems should index authority-linked projections, not invent new
  message identities.
- `conversation_id` is a conversation container id, not a message id.
- Threads remain message-anchored conversation containers; their messages still need their own
  `authority_message_id` values.

## Open Risks

- Cross-node total ordering is still not a first-class contract; this spec only stabilizes identity
  and ownership semantics.
- Some existing projections still rely on payload-level conventions instead of first-class columns.
- Multi-tenant rollout will require adding `group_id` to more persisted surfaces than currently
  exist.

## Source Journals

- `docs/journal/2026-03-19-distributed-p2p-pipeline.md`
- `docs/journal/2026-03-13-team-shared-thread-canonical-replies.md`
- `docs/journal/2026-05-04-distributed-message-metadata-contract-phase1.md`
