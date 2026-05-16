# Message Storage Tiering

## Problem

AgentHub stores message-like data across SQLite authority tables, per-agent event SQLite databases,
and a LanceDB archive/search layer. This is enough for correctness and search, but it mixes several
different access patterns:

- authority writes need transactional constraints and idempotency;
- channel and mailbox reads need ordered range scans by channel, actor, agent, run, and cursor;
- archive/search needs long-lived document storage and text/semantic retrieval;
- distributed nodes need cacheable delivery projections without redefining message truth.

SQLite can continue to own relational authority rows, and LanceDB can continue to own archive/search
documents, but neither is the ideal physical structure for hot ordered delivery indexes. The next
storage boundary should introduce a dedicated ordered delivery index, with RocksDB as the first
candidate implementation.

## Scope

- Define the stable three-layer message storage architecture:
  - SQLite authority;
  - RocksDB ordered delivery index;
  - LanceDB archive/search.
- Define which message metadata belongs in each layer.
- Define key shapes for ordered channel, agent, run, inbox, ack, and cursor access.
- Define consistency and recovery rules for dual writes across the three layers.
- Define the minimum next PR boundary for adding a RocksDB-backed delivery index without replacing
  existing SQLite authority tables.

## Non-Goals

- Replacing Team, run, permission, node, or group authority rows in SQLite.
- Replacing LanceDB archive/search with RocksDB.
- Moving full message bodies exclusively into RocksDB.
- Introducing cross-node RocksDB replication.
- Making RocksDB mandatory for existing single-node installations in the first rollout.
- Compressing SQLite rows with `sqlite-zstd` in the same PR.

## Architecture

### 1) Storage Layer Responsibilities

AgentHub should split message persistence into three layers.

`SQLite authority`

- owns canonical relational truth;
- enforces idempotency and ownership constraints;
- stores Team, run, mailbox, node, group, permission, and session metadata;
- remains the source of truth for state transitions such as `pending`, `delivered`, `dead_letter`,
  and task/run lifecycle states.

`RocksDB ordered delivery index`

- stores compact ordered read models for hot delivery and replay paths;
- supports prefix/range scans by channel, agent, run, inbox, and actor cursor;
- stores references to archive documents and small immutable routing metadata;
- is rebuildable from SQLite authority rows plus LanceDB/archive metadata;
- must not become a second authority layer.

`LanceDB archive/search`

- stores canonical searchable message documents;
- owns full-text and later vector/hybrid retrieval;
- stores full body/search payloads and archive projection metadata;
- remains the target for message search APIs.

### 2) RocksDB Delivery Index Role

RocksDB is an ordered index and cache, not the canonical message store.

The index value should contain enough data for low-latency list and delivery paths without requiring
one LanceDB lookup per row:

- stable `message_id`;
- `archive_document_id`;
- `created_at`;
- `source_kind`;
- `message_kind`;
- `authority_message_id`;
- `correlation_id`;
- optional `group_id`;
- optional `run_id`;
- optional `conversation_id`;
- optional `agent_id`;
- compact delivery state when the key is ack/cursor scoped.

Large payloads, searchable body text, tool output bodies, and full JSON payloads belong in LanceDB
or SQLite authority rows, not RocksDB values.

### 3) Keyspace Layout

The first RocksDB backend should use explicit column families or prefix namespaces. The physical
choice may be backend-specific, but the logical keyspace must stay stable.

Recommended logical keys:

```text
msg/by_channel/<group_id>/<channel_id>/<sort_id> -> MessageRef
msg/by_agent/<agent_id>/<sort_id> -> MessageRef
msg/by_run/<run_id>/<sort_id> -> MessageRef
msg/by_id/<message_id> -> MessageRef
inbox/by_actor/<actor_id>/<sort_id> -> MessageRef
ack/by_actor/<actor_id>/<channel_id>/<sort_id> -> AckState
cursor/by_actor/<actor_id>/<channel_id> -> CursorState
```

`message_kind` is a compact presentation hint such as `text`, `tool_call`, `event`, `thought`, or
`system`. It lets ordered list views render icons and summaries without fetching the full archive
document.

`inbox/by_actor` intentionally keeps `run_id` in `MessageRef` instead of the key prefix so one actor
can scan a unified chronological inbox across multiple runs. If a future query needs a run-scoped
actor inbox, it should add a separate secondary prefix instead of weakening the unified inbox order.

`sort_id` must be monotonic and stable enough for replay and must be collision-safe when one prefix
aggregates rows from multiple authority tables. Preferred shapes are either a UUIDv7-derived
timestamp/order tuple or a composite fixed-width encoding such as
`timestamp:source_kind:source_row_id`. String keys must use an encoding that preserves bytewise
order.

`message_id` is the logical delivery id for the delivery index. `archive_document_id` points to the
LanceDB document. They are related but not interchangeable.

### 4) Write Path

The canonical write path remains authority-first:

1. Write or update SQLite authority rows inside the existing transaction boundary.
2. Build deterministic archive documents and append/upsert them into LanceDB.
3. Build deterministic RocksDB index mutations from the committed authority row and archive
   document id.
4. Commit RocksDB mutations with `WriteBatch`.

If archive or RocksDB writes fail after the SQLite authority commit, user-visible authority writes
must not be rolled back. Recovery is driven by deterministic re-indexing from SQLite authority rows.

For flows where LanceDB append happens asynchronously, RocksDB may store a pending archive reference
state, but it must preserve enough authority metadata for a later repair job to fill the
`archive_document_id`.

### 5) Read Path

Hot ordered reads should prefer RocksDB once the backend is enabled:

- channel timeline reads use `msg/by_channel`;
- agent transcript windows use `msg/by_agent`;
- run-scoped event windows use `msg/by_run`;
- actor inbox polling uses `inbox/by_actor`;
- ack/cursor reads use `ack/by_actor` and `cursor/by_actor`.

Search APIs continue to use LanceDB. Search results may optionally hydrate delivery state from
RocksDB when the UI needs unread/ack/cursor hints, but search ranking and body matching stay in the
archive layer.

SQLite remains the fallback compatibility path until the RocksDB index has been built and validated.

### 6) Rebuild And Repair

The RocksDB index must be fully rebuildable.

Inputs:

- SQLite authority rows:
  - `team_conversation_messages`;
  - `team_actor_messages`;
  - `team_run_events`;
  - main and per-agent `agent_events`;
  - channel replica rows when present.
- LanceDB archive documents or deterministic archive document-id builders.

Required repair operations:

- dry-run index scan that reports expected key counts per namespace;
- rebuild namespace for one team, channel, agent, run, or actor;
- rebuild all delivery indexes from SQLite authority rows;
- verify RocksDB refs point to existing archive document ids when archive is enabled;
- detect and report orphan RocksDB refs, with an explicit prune mode that deletes refs not backed by
  authority rows.

### 7) Distributed Node Semantics

In distributed mode, `main` remains the authority node. RocksDB on non-main nodes is a local delivery
projection/cache.

Node-local RocksDB may store:

- inbox rows addressed to local actors;
- channel context needed for local execution;
- agent-local transcript windows;
- cursor/ack projections for local runtime consumption.

Node-local RocksDB must preserve authority references:

- `run_id`;
- `conversation_id`;
- `authority_message_id`;
- `correlation_id`;
- optional `group_id`;
- `source_node_id`;
- `target_node_id`;
- `idempotency_key` when needed for replay/debugging.

If a node-local RocksDB index diverges from `main`, `main` wins. Gossip may help discover node
membership and routing hints, but it must not authorize or redefine message authority.

## Contracts

### 1) Authority Contract

- SQLite authority rows are the source of truth for message identity, ownership, delivery state,
  and idempotency.
- RocksDB rows are projections derived from SQLite authority rows.
- LanceDB rows are archive/search projections derived from authority rows and ACP aggregation.
- Any conflict between SQLite and RocksDB is resolved by rebuilding RocksDB from SQLite.

### 2) Delivery Index Contract

- RocksDB keys must be deterministic and prefix-range friendly.
- Every RocksDB `MessageRef` must include a stable `message_id` and enough authority metadata to
  reconcile it.
- RocksDB values must be compact; full message bodies stay out of the delivery index.
- Multi-key mutations for one logical write must use `WriteBatch`.
- Index writes must be idempotent; replaying the same authority row produces the same keys and
  values.

### 3) Archive/Search Contract

- LanceDB remains the canonical archive/search backend for message documents.
- RocksDB may reference LanceDB `document_id`, but must not implement search ranking.
- Archive document ids must be deterministic so RocksDB can be repaired after missed dual writes.
- Search APIs must continue to use the message archive abstraction.

### 4) Sort And Cursor Contract

- `sort_id` must preserve chronological order within one prefix.
- Cursor state must be actor/channel scoped, not global.
- A cursor update must not imply message acknowledgement unless the caller explicitly performs ack.
- Ack state must preserve authority references so it can be reconciled against SQLite mailbox state.

### 5) Configuration Contract

- The first rollout must be opt-in.
- Default installations continue to use the existing SQLite-backed read path.
- Configuration should distinguish:
  - archive backend (`lancedb`);
  - delivery index backend (`sqlite` compatibility path or `rocksdb`);
  - delivery index path;
  - rebuild/repair mode.
- Enabling RocksDB must not require running `sqlite-zstd`.

### 6) Migration Contract

- Migration starts as dual-write plus backfill, not destructive movement.
- Historical SQLite rows remain readable during the transition.
- Backfill must be idempotent and resumable by prefix scope.
- Rebuild must be safe to interrupt.
- Rollback path is disabling the RocksDB delivery index and falling back to SQLite read paths; no
  authority data should be lost.

## Validation Matrix

- Unit tests for delivery key encoding:
  - bytewise order preserves `sort_id` order;
  - group/channel/agent/run prefixes do not collide;
  - deterministic keys are stable across replay.
- Unit tests for `MessageRef` serialization:
  - preserves `archive_document_id`;
  - preserves authority references;
  - rejects or reports malformed values.
- RocksDB backend tests:
  - open/create column families or namespaces;
  - append one message with all secondary indexes in one batch;
  - range scan by channel;
  - range scan by agent;
  - range scan by run;
  - cursor update/read;
  - ack update/read.
- Integration tests for dual-write:
  - SQLite authority write succeeds when RocksDB is disabled;
  - SQLite authority write still succeeds when RocksDB write fails after commit;
  - repair job rebuilds missing RocksDB keys from SQLite rows.
- Archive/search integration tests:
  - RocksDB `archive_document_id` points to a LanceDB document written by the archive layer;
  - search still uses LanceDB and returns the same document metadata.
- Distributed tests:
  - node-local RocksDB cache preserves `source_node_id`, `target_node_id`, and `idempotency_key`;
  - rebuilding a node-local cache from main authority rows restores the same delivery window.
- Compatibility tests:
  - default config uses SQLite read path;
  - opt-in RocksDB config uses delivery index read path;
  - disabling RocksDB falls back to SQLite without data migration.

## Operational Notes

- RocksDB introduces a native storage dependency and should be isolated behind a feature flag or
  backend adapter boundary until CI/prebuild coverage is stable.
- Operators need a debug surface to inspect key counts, cursor state, and orphan refs without
  dumping message bodies.
- Backups must include SQLite authority data and LanceDB archive data. RocksDB can be treated as
  rebuildable, but keeping it in backups can speed restore.
- Compaction and write-buffer settings should be conservative initially; delivery-index writes are
  small but high-frequency.
- `sqlite-zstd` remains a separate optional optimization. It should not be combined with the first
  RocksDB delivery-index PR because both change persistence behavior and rollback reasoning.

## Open Risks

- RocksDB build and packaging may add platform-specific CI/prebuild work.
- Dual-write ordering can create temporary archive/index gaps if LanceDB or RocksDB fails after a
  SQLite authority commit.
- Poor key design will make future unread, group, or actor-scoped queries expensive to add.
- Treating RocksDB values as message bodies would duplicate archive storage and make privacy/debug
  controls harder.
- Node-local RocksDB indexes can drift from `main` unless repair/reconciliation is treated as part
  of the contract.

## Source Journals

- [docs/journal/2026-05-04-lancedb-message-archive-phase1.md](../journal/2026-05-04-lancedb-message-archive-phase1.md)
- [docs/journal/2026-05-05-message-archive-team-conversation-dual-write.md](../journal/2026-05-05-message-archive-team-conversation-dual-write.md)
- [docs/journal/2026-05-05-message-archive-team-search-api.md](../journal/2026-05-05-message-archive-team-search-api.md)
- [docs/journal/2026-05-05-message-archive-team-migration.md](../journal/2026-05-05-message-archive-team-migration.md)
- [docs/journal/2026-05-06-message-archive-step-lifecycle-run-events.md](../journal/2026-05-06-message-archive-step-lifecycle-run-events.md)
- [docs/journal/2026-05-06-task-message-correlation-authority.md](../journal/2026-05-06-task-message-correlation-authority.md)
- [docs/journal/2026-05-06-team-actor-message-group-id.md](../journal/2026-05-06-team-actor-message-group-id.md)
