# Logical Message Metadata Contract

## Problem

AgentHub already carries message identity and delivery metadata across multiple layers:

- canonical conversation rows on `main`;
- actor mailbox rows on `main`;
- remote relay envelopes between `main` and `node`;
- node-local replica/cache rows;
- search/archive projections.

Today these layers already share useful fields such as `conversation_id`,
`authority_message_id`, `correlation_id`, `broadcast_id`, and `idempotency_key`, while `run_id`
already serves as the mailbox/execution partition key on delivery-facing authority rows and
projections. The system does not yet define one canonical ownership contract for these fields.
Without that contract, future work on distributed replay, search, gossip-driven node membership,
and multi-tenant isolation can drift into incompatible metadata semantics.

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

Current authority-layer fields are split by authority surface:

- `conversation_id`
- `authority_message_id`
- `correlation_id`

Current execution/mailbox authority field:

- `run_id`

Future compatibility field:

- `group_id`

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
- optional `group_id` when the source authority row already carries a live group boundary

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
- `run_id` is the canonical execution-partition identity for mailbox and execution surfaces.

These fields serve different purposes and must not be conflated:

- `authority_message_id` identifies one message;
- `correlation_id` identifies one intent chain that may span multiple messages;
- `conversation_id` identifies one human-visible conversation container;
- `run_id` identifies one execution scope on delivery-facing authority rows and projections.

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
| `team_conversation_messages` | authority row | `conversation_id`, `authority_message_id`, `correlation_id`, optional `group_id` | canonical human-visible message content; `correlation_id` and the Team-derived nullable `group_id` are persisted as first-class authority columns; does not currently persist `run_id` |
| `team_actor_messages` | authority row | `run_id`, sender/recipient actor ids, effective `idempotency_key`, optional `group_id` | canonical delivery state; nullable `group_id` inherits from the owning run |
| `team_channel_message_replicas` | replica row | `run_id`, `conversation_id`, `authority_message_id`, `correlation_id`, optional `group_id` | node-relevant channel cache only; `correlation_id` and nullable `group_id` are persisted as first-class projection columns |
| remote relay route metadata | delivery metadata | `source_node_id`, `target_node_id`, optional `broadcast_id`, optional `correlation_id`, effective `idempotency_key` | transport/debug only |
| archive/search document | projection row | `run_id`, `conversation_id`, `authority_message_id`, `correlation_id`, optional `group_id` | must point back to `main` authority; Team conversation, actor, channel-replica, and run-event documents preserve `group_id` when supplied by the owning authority source; Team search accepts `group_id` as a projection filter |

`group_id` is intentionally optional in the current surface matrix because historical rows may not
carry it. New Team authority and projection paths now preserve the owning group when available, but
routing must still treat missing `group_id` as `unknown` until a later enforcement rollout.

### 7) `group_id` Physical Rollout Plan

`group_id` must become live in authority rows before it becomes a required projection filter. The
rollout order is:

1. Define the canonical group authority source.
   - Initial group ownership should be derived from Team ownership, not from node-local state.
   - Before a dedicated groups table exists, `team_definitions.owner_user_id` is the compatibility
     boundary for single-user installations, but it must not be renamed or treated as the final
     group id.
   - For node registry rows, the reviewed assignment source is the root/admin write path on
     `main`; node-local mirrors and gossip observations must not invent group assignments.
2. Add nullable `group_id` to control-plane authority rows.
   - `team_definitions`, `team_tasks`, and `team_runs` should carry the Team group boundary first.
   - `node` registry authority rows carry the assigned group boundary through the main-owned create
     and update API before routing enforces it.
3. Propagate `group_id` into message authority rows.
   - `team_conversation_messages` inherits from the owning task, which already inherits from the
     owning Team.
   - `team_actor_messages` inherits from the owning run.
4. Propagate `group_id` into projection rows.
   - `team_channel_message_replicas` copies it from the authority message or run context.
   - archive/search documents should preserve it when the authority source supplies it.
   - Team archive search accepts `group_id` as a normalized filter so future group-scoped search
     can rely on the same projection field.
5. Enforce routing boundaries after data is populated.
   - mailbox, channel fan-out, and remote relay paths should reject cross-group routing unless a
     future bridge contract explicitly permits it.

Each phase must be backward compatible with existing rows where `group_id` is absent. Reads should
treat missing `group_id` as `unknown`, not as permission to cross group boundaries.

## Validation Matrix

- `cargo test internal_grpc_mailbox_send_persists_channel_replica_history -- --nocapture`
- `cargo test internal_grpc_mailbox_send_rejects_channel_replica_payload_without_correlation_id -- --nocapture`
- `cargo test -p agenthub-db init_db_adds_task_message_correlation_id_and_backfills_existing_rows -- --nocapture`
- `cargo test -p agenthub append_task_conversation_message_persists_correlation_id_column -- --nocapture`
- `cargo test -p agenthub-db init_db_adds_and_backfills_team_conversation_message_group_ids -- --nocapture`
- `cargo test -p agenthub append_task_conversation_message_persists_authority_group_id -- --nocapture`
- `cargo test -p agenthub-db init_db_adds_and_backfills_team_actor_message_group_ids -- --nocapture`
- `cargo test -p agenthub send_actor_message_persists_authority_group_id -- --nocapture`
- `cargo test -p agenthub-db init_db_adds_and_backfills_channel_replica_group_ids -- --nocapture`
- `cargo test -p agenthub internal_grpc_mailbox_send_persists_channel_replica_history -- --nocapture`
- `cargo test -p agenthub create_and_patch_agent_node_preserves_main_owned_group_id -- --nocapture`
- `cargo test -p agenthub team_message_search_api_uses_archive_with_team_scope -- --nocapture`
- `cargo test -p agenthub append_run_event_dual_writes_created_event_to_archive -- --nocapture`
- `cargo test -p agenthub migrate_team_messages_to_archive_covers_team_message_tables -- --nocapture`
- `cargo test remote_actor_messages_relay_success_marks_message_delivered -- --nocapture`
- `cargo test bidirectional_actor_grpc_pipeline_relays_seeded_messages_between_in_process_states -- --nocapture`

## Operational Notes

- Prefer `main` authority plus node-local relevant-message cache over multi-node message truth
  synchronization.
- Node-local archive/search systems should index authority-linked projections, not invent new
  message identities.
- Treat `group_id` as a compatibility target for historical rows even though new Team authority,
  node-registry, and searchable projection paths now preserve it when available.
- Do not enforce cross-group routing until `group_id` is populated on both node registry authority
  rows and message authority rows.
- `conversation_id` is a conversation container id, not a message id.
- Threads remain message-anchored conversation containers; their messages still need their own
  `authority_message_id` values.

## Open Risks

- Cross-node total ordering is still not a first-class contract; this spec only stabilizes identity
  and ownership semantics.
- Some existing legacy rows still rely on payload-level conventions instead of first-class columns.
- Multi-tenant rollout still needs policy enforcement and a dedicated groups table; this contract
  only establishes the current authority and projection ownership boundary.

## Source Journals

- `docs/journal/2026-03-19-distributed-p2p-pipeline.md`
- `docs/journal/2026-03-13-team-shared-thread-canonical-replies.md`
- `docs/journal/2026-05-04-distributed-message-metadata-contract-phase1.md`
- `docs/journal/2026-05-06-message-archive-group-id-projection.md`
- `docs/journal/2026-05-06-channel-replica-correlation-projection.md`
- `docs/journal/2026-05-06-task-message-correlation-authority.md`
- `docs/journal/2026-05-06-group-id-rollout-plan.md`
- `docs/journal/2026-05-06-team-authority-group-id.md`
- `docs/journal/2026-05-06-node-registry-group-id.md`
- `docs/journal/2026-05-07-metadata-projection-contract-closure.md`
