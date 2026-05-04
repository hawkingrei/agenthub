# Distributed Node Registry And Gossip

## Problem

AgentHub now supports a `main` + `node` topology, but the system still needs a stable contract for:

- which node stores authority registry state;
- what every node may keep locally in SQLite;
- what gossip is allowed to synchronize;
- how future multi-tenant or multi-group boundaries should apply to nodes and routing.

Without a stable registry contract, it is too easy to let node-local mirrors, gossip observations,
and control-plane records drift into competing sources of truth.

## Scope

- define `group` as the outer trust/routing boundary for node membership;
- define canonical node-registry ownership;
- define node-local mirrored state;
- define what gossip may synchronize and what remains out of scope for gossip.

## Non-Goals

- full multi-tenant product UX;
- replacing `main` as the control-plane authority;
- using gossip to transport mailbox or conversation payloads;
- defining every low-level heartbeat packet format in this step.

## Architecture

### 1) Group

`group` is the outer distributed coordination boundary.

It should eventually own:

- nodes;
- teams;
- bootstrap/trust domain;
- cluster-scoped routing policy.

`group_id` should be the long-term isolation key for distributed routing and multi-tenant work.

### 2) Main Authority Registry

`main` owns the canonical distributed registry rows for:

- node membership;
- node role (`main` / `node`);
- canonical endpoint metadata;
- capability summary;
- last reconciled status;
- group membership.

When node-local mirrors disagree with `main`, `main` wins.

### 3) Node-Local Mirror

Each `node` may store a local SQLite mirror for:

- self identity;
- replicated node membership snapshot;
- local health/capability observations;
- local execution caches that need node-aware routing context.

This mirror exists for runtime usability and partial offline recovery, not for authority.

### 4) Gossip As Metadata Plane

Gossip should only synchronize metadata-plane information:

- node membership discovery;
- heartbeat / last-seen observations;
- capability/runtime summary;
- group membership hints;
- compact invalidation or refresh descriptors.

Gossip must not synchronize:

- mailbox truth;
- canonical conversation messages;
- actor delivery ack truth;
- thread/channel authority rows.

## Contracts

### 1) Main Authority Contract

`main` stores the authority rows for distributed coordination state.

That includes:

- canonical node-registry rows;
- mailbox authority rows;
- canonical conversation authority rows;
- team/run/task coordination rows.

### 2) Node Mirror Contract

`node` stores:

- local runtime state;
- relevant message caches;
- registry mirrors and observations;
- archive/search projections when needed.

Node-local mirrored rows are allowed to be incomplete, eventually refreshed, or rebuilt.

### 3) Gossip Contract

Gossip is a metadata plane only.

It may update local observations, but it must not redefine `main` authority rows.

### 4) Group Boundary Contract

- `group_id` is the intended long-term trust and routing domain key.
- `node_id` should become unique within one `group_id` as the group rollout becomes physical
  schema, not just contract direction.
- mailbox or execution routing should not cross `group_id` without an explicit future bridge
  contract once `group_id` becomes live on those paths.
- gossip membership exchange should be scoped by `group_id` once registry surfaces carry it
  consistently.

### 5) Message Interaction Contract

This spec depends on `main` authoritative message storage:

- `main` keeps canonical message truth;
- `node` keeps only relevant local caches;
- node-local cache reconciliation should use authority references such as
  `authority_message_id` and `conversation_id`.

## Validation Matrix

- registry/metadata contract review against `docs/features/distributed-node-architecture.md`
- focused relay and internal gRPC tests proving message delivery still depends on `main` authority:
  - `cargo test remote_actor_messages_relay_success_marks_message_delivered -- --nocapture`
  - `cargo test bidirectional_actor_grpc_pipeline_relays_seeded_messages_between_in_process_states -- --nocapture`

## Operational Notes

- Prefer `main` authority plus node-local mirrors over attempting cluster-wide registry truth via
  gossip alone.
- Node-local SQLite mirrors may be versioned independently, but they must remain reconcilable
  against `main`.
- Archive/search rollouts should treat node-local data as cache/projection, not canonical truth.
- `group_id` remains a forward-compatibility boundary in this phase; the live runtime does not yet
  enforce it across every node/message surface.

## Open Risks

- Node-registry schema still needs a concrete physical rollout plan.
- Group/tenant isolation exists here as a contract direction before it exists everywhere in the
  live schema.
- Gossip conflict resolution still needs a concrete version/epoch policy once node mirrors become
  mutable.

## Source Journals

- `docs/journal/2026-03-19-distributed-p2p-pipeline.md`
- `docs/journal/2026-05-04-distributed-message-metadata-contract-phase1.md`
