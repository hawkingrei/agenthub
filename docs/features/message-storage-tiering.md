# Message Storage Tiering

## Problem

AgentHub stores message-like data across SQLite authority tables, per-agent event SQLite databases,
and a LanceDB archive/search layer. This is enough for correctness and search, but it mixes several
different access patterns and stores message bodies inefficiently:

- authority writes need transactional constraints and idempotency;
- channel and mailbox reads need ordered range scans by channel, actor, agent, run, and cursor;
- archive/search needs long-lived document storage and text/semantic retrieval;
- distributed nodes need cacheable delivery projections without redefining message truth;
- message bodies (chat text, tool output, JSON payloads) are stored uncompressed in SQLite authority
  rows and grow unbounded with history.

SQLite is a good relational authority and LanceDB is a good search/archive layer, but neither is the
ideal physical structure for two things: hot ordered delivery indexes, and compact at-rest storage of
high-volume message bodies. The storage boundary should introduce a dedicated ordered delivery index
and a compressed body store, both backed by RocksDB.

RocksDB is a natural fit for both: its LSM SST blocks are block-compressed (zstd-capable), so
aggregating many small chat messages into a block compresses far better than per-row compression of
short text, and its ordered keyspace serves prefix-range delivery reads directly. This lets SQLite
authority rows shrink to metadata-only and moves the bulky body bytes into a compressed engine-managed
store.

## Scope

- Define the stable three-layer message storage architecture:
  - SQLite authority (metadata only);
  - RocksDB ordered delivery index plus compressed body store;
  - LanceDB archive/search.
- Define which message metadata belongs in each layer, and the boundary between the body-free delivery
  index and the compressed body column family.
- Define key shapes for ordered channel, agent, run, inbox, ack, and cursor access, plus the body key.
- Define the RocksDB SST compression configuration for the body column family (zstd, bottommost level).
- Define consistency, durability, and recovery rules across the three layers.
- Define the staged migration that moves message bodies out of SQLite authority rows into the RocksDB
  body store without destructive movement.

## Non-Goals

- Replacing Team, run, permission, node, or group authority rows in SQLite. SQLite remains the
  relational authority; only the message *body* moves out.
- Replacing LanceDB archive/search with RocksDB.
- Introducing cross-node RocksDB replication.
- Making the RocksDB backend the default in the first rollout; it is opt-in until validated.
- Hand-rolling a per-row body codec for the RocksDB body store. Compression is engine-managed at the
  SST level; the existing `agenthub-agent-event-codec` per-row zstd path stays only for the legacy
  SQLite-body compatibility window.
- Compressing the remaining SQLite metadata rows with `sqlite-zstd`; metadata rows are small and out
  of scope.

## Architecture

### 1) Storage Layer Responsibilities

AgentHub splits message persistence into three layers.

`SQLite authority` (metadata only)

- owns canonical relational truth: message identity, ownership, delivery state, idempotency;
- stores Team, run, mailbox, node, group, permission, and session metadata;
- remains the source of truth for state transitions such as `pending`, `delivered`, `dead_letter`,
  and task/run lifecycle states;
- after migration, does not store the message body; it stores a body locator (`message_id`) plus the
  metadata needed to render lists and reconcile state.

`RocksDB` (ordered delivery index + compressed body store)

- delivery index: compact ordered read models for hot delivery and replay paths, supporting
  prefix/range scans by channel, agent, run, inbox, and actor cursor; this portion is derived from
  SQLite authority rows and is rebuildable;
- body store: a dedicated column family holding the full message body, compressed by RocksDB SST block
  compression; this portion is primary data for body bytes, not a derived cache;
- the delivery index must not become a second relational authority; the body store is authoritative
  only for body bytes, never for identity, ownership, or delivery state.

`LanceDB archive/search`

- stores searchable message documents and owns full-text and later vector/hybrid retrieval;
- remains the target for message search APIs;
- is an eventually-consistent projection: it may lag and may not contain every message. This is
  accepted. LanceDB is not relied on as the durable source of body bytes.

### 2) RocksDB Roles: Index vs Body

RocksDB hosts two distinct concerns in two column families with different tuning.

Delivery index column family (`cf_index`)

- ordered, body-free `MessageRef` values for low-latency list and delivery paths;
- light or no compression (the values are already small);
- a `MessageRef` value carries enough to render a row without a body fetch:
  - stable `message_id`;
  - `archive_document_id`;
  - `created_at`;
  - `source_kind`;
  - `message_kind`;
  - `authority_message_id`;
  - `correlation_id`;
  - optional `group_id`, `run_id`, `conversation_id`, `agent_id`;
- compact delivery state when the key is ack/cursor scoped.

The message-store foundation exposes `MessageIndexProjection` as the typed boundary for writing these
body-free refs. A projection always writes the direct `msg/by_id/<message_id>` lookup and may write
channel, agent, run, and actor-inbox ordered prefixes for the same `MessageRef`. This API is the
storage boundary only: callers must still derive projections from SQLite authority metadata, and
authority-derived rebuild/repair remains the correctness boundary before ordered reads can become
production-facing.

Body column family (`cf_body`)

- maps `authority_message_id` to the full body payload (text, tool output, JSON);
- keyed by the canonical logical-message identity, not a per-delivery id, so one logical message stores
  exactly one body even when it fans out to multiple actors/channels;
- aggressively compressed by SST block compression (see §4);
- large payloads and full bodies live here only — never inlined into a `MessageRef`.

Keeping bodies out of the index keeps ordered scans cheap (small ref blocks) and confines body
decompression to explicit single-message fetches. `authority_message_id` is the canonical logical
message identity defined in
[logical-message-metadata-contract.md](logical-message-metadata-contract.md); a per-delivery
`message_id` (one per delivery/fan-out attempt) identifies an index row but is never the body key, so
fan-out never duplicates a body.

### 3) Keyspace Layout

RocksDB uses explicit column families. The physical layout may be backend-specific, but the logical
keyspace must stay stable.

Index column family (`cf_index`):

```text
msg/by_channel/<group_id>/<channel_id>/<sort_id> -> MessageRef
msg/by_agent/<agent_id>/<sort_id> -> MessageRef
msg/by_run/<run_id>/<sort_id> -> MessageRef
msg/by_id/<message_id> -> MessageRef
inbox/by_actor/<actor_id>/<sort_id> -> MessageRef
ack/by_actor/<actor_id>/<channel_id>/<sort_id> -> AckState
cursor/by_actor/<actor_id>/<channel_id> -> CursorState
```

Body column family (`cf_body`):

```text
body/by_message/<authority_message_id> -> MessageBody   (engine-compressed, one per logical message)
```

`message_kind` is a compact presentation hint such as `text`, `tool_call`, `event`, `thought`, or
`system`. It lets ordered list views render icons and summaries without fetching the body.

`inbox/by_actor` intentionally keeps `run_id` in `MessageRef` instead of the key prefix so one actor
can scan a unified chronological inbox across multiple runs. If a future query needs a run-scoped
actor inbox, it should add a separate secondary prefix instead of weakening the unified inbox order.

`sort_id` must be monotonic and stable for replay and must be collision-safe when one prefix aggregates
rows from multiple authority tables. It should be an authority-assigned monotonic value rather than a
wall-clock timestamp, because multiple authority tables and (later) multiple nodes cannot rely on clock
ordering. The preferred shape is a fixed-width encoding of an authority sequence per logical stream
(for example `authority_seq:source_kind:source_row_id`); a UUIDv7-derived tuple is acceptable for
single-node single-source streams. String keys must use a bytewise-order-preserving encoding.

`message_id` is the per-delivery id used in index rows; `authority_message_id` is the canonical
logical-message identity and the `cf_body` key; `archive_document_id` points to the LanceDB document.
They are related but not interchangeable: a single `authority_message_id` (one body) may have several
delivery `message_id` index rows fanned out to different actors/channels.

### 4) Body Compression (SST)

The body column family relies on RocksDB SST block compression rather than an application codec.

- `cf_body` compression is `zstd`; `bottommost_compression` is the same `zstd` algorithm at a higher
  zstd *compression level* (not an LSM level) on the bottommost LSM level, so cold, rarely-rewritten
  body data compacts to a smaller footprint than hot levels.
- `block_size` for `cf_body` is tuned so multiple small messages aggregate into one block, giving zstd
  cross-message context within a block. The index column family keeps a small block size for scan
  locality.
- Bodies are stored as raw bytes; the engine owns compression. The legacy per-row
  `agenthub-agent-event-codec` zstd path is used only for bodies that still live in SQLite during the
  migration window, not for `cf_body`.

A trained zstd dictionary (engine auto-trained per SST, or an externally-trained preset) would further
improve the ratio on very short messages, but it is **deferred**: the first rollout uses plain zstd
only. If a dictionary is added later, prefer the engine auto-trained per-SST dictionary because it is
stored inside each SST and needs no version management.

### 5) Write Path

The canonical write path is authority-first, but failure semantics differ for **derived** state (the
index, always rebuildable) and **primary** state (the body, which after Phase 2 lives only in
RocksDB). The body must have a durable home before `cf_body` acknowledges, so the body is staged inside
the SQLite authority transaction.

1. In the existing SQLite authority transaction: write/update the metadata rows **and** insert the body
   into a SQLite body outbox (`message_body_outbox`, keyed by `authority_message_id`). The body is
   durably committed with the metadata.
2. Build deterministic archive documents and append/upsert them into LanceDB.
3. In one RocksDB `WriteBatch`: put the body into `cf_body` and put the derived index refs into
   `cf_index`.
4. Once `cf_body` durably acknowledges the body, delete the outbox row for that `authority_message_id`.

Failure handling:

- The delivery **index** is derived. If the `cf_index` write fails after the authority commit, the
  authority write is not rolled back; the index is re-derived from SQLite metadata. To bound the
  window, each ordered prefix tracks the authority sequence it has been indexed up to (a per-prefix
  high-water mark); a read that sees the index lagging the authority sequence falls back to SQLite or
  triggers read-repair, so a freshly written message is never silently missing from the hot path.
- The message **body** is primary. A `cf_body` write failure is a hard failure for the body path, but
  it still does not roll back the metadata commit, because the body survives in the SQLite outbox. A
  background drainer retries outbox rows into `cf_body` and clears them on durable ack. The outbox is
  the source of truth for "bodies not yet confirmed in `cf_body`", so no body is lost even if the
  process crashes between the authority commit and the `cf_body` ack.

During Phase 1 the legacy SQLite body column also still holds the body, so the outbox is effectively
redundant; normal conversation reads use that SQLite compatibility copy rather than `cf_body`. From
Phase 2 onward the outbox is the only non-RocksDB durable home for an unconfirmed body.

For flows where LanceDB append happens asynchronously, the index may store a pending
`archive_document_id` state, but it must preserve enough authority metadata for a later repair job to
fill it.

### 6) Read Path

Hot ordered reads prefer RocksDB once the backend is enabled:

- channel timeline reads use `msg/by_channel`;
- agent transcript windows use `msg/by_agent`;
- run-scoped event windows use `msg/by_run`;
- actor inbox polling uses `inbox/by_actor`;
- ack/cursor reads use `ack/by_actor` and `cursor/by_actor`.

Ordered reads return body-free `MessageRef` rows. Rendering or returning a full message body fetches
`body/by_message/<authority_message_id>` from `cf_body` on demand (one block decompress per body).

Search APIs continue to use LanceDB. Search results may optionally hydrate delivery state from the
index when the UI needs unread/ack/cursor hints, but search ranking and body matching stay in the
archive layer.

SQLite remains the body read path throughout Phase 1. RocksDB ordered-index reads and body hydration
remain future work until the backend has been validated and the SQLite body column can be dropped.

### 7) Rebuild And Repair

The delivery index is fully rebuildable **from SQLite authority metadata alone**. The body store is
primary data and is not rebuildable from SQLite once the SQLite body column is dropped. `cf_body` is
therefore never an input to index rebuild — only to integrity checks — so partial body loss can never
block an index rebuild.

Index rebuild inputs:

- SQLite authority rows: `team_conversation_messages`, `team_actor_messages`, `team_run_events`, main
  and per-agent `agent_events`, channel replica rows when present;
- deterministic `archive_document_id` builders (the index stores the id; it does not need to read the
  archive document to rebuild).

Required operations:

- dry-run index scan that reports expected key counts per namespace;
- rebuild index namespace for one team, channel, agent, run, or actor from SQLite metadata;
- rebuild all delivery indexes from SQLite metadata;
- integrity check (not rebuild): verify each index ref has a matching `cf_body` entry and, when archive
  is enabled, an existing archive document id;
- detect and report orphan index refs, with an explicit prune mode that deletes refs not backed by
  authority rows;
- detect index refs whose `cf_body` entry is missing (a body-loss signal that the SQLite body outbox or
  a backup restore must resolve; LanceDB archive may be used as a best-effort body recovery source but
  is not authoritative and may be incomplete).

### 8) Distributed Node Semantics

In distributed mode, `main` remains the authority node. RocksDB on non-main nodes is a local delivery
projection and a local body cache for messages relevant to that node.

Node-local RocksDB may store:

- inbox rows addressed to local actors;
- channel context needed for local execution;
- agent-local transcript windows and their bodies;
- cursor/ack projections for local runtime consumption.

Node-local entries must preserve authority references: `run_id`, `conversation_id`,
`authority_message_id`, `correlation_id`, optional `group_id`, `source_node_id`, `target_node_id`, and
`idempotency_key` when needed for replay/debugging.

If a node-local index diverges from `main`, `main` wins. Node-local body copies are caches; the durable
body authority is `main`'s `cf_body`. Gossip may help discover node membership and routing hints, but
it must not authorize or redefine message authority.

## Contracts

### 1) Authority Contract

- SQLite authority rows are the source of truth for message identity, ownership, delivery state, and
  idempotency.
- RocksDB `cf_index` rows are projections derived from SQLite authority rows; conflicts are resolved by
  rebuilding the index from SQLite metadata alone (`cf_body` is never an index-rebuild input).
- RocksDB `cf_body` is the authoritative store of message body bytes after migration; it is primary
  data, must be backed up, and is not rebuildable from SQLite metadata alone. Until `cf_body` durably
  acknowledges a body, that body remains in the SQLite `message_body_outbox`, so an authority commit
  never leaves a body without a durable home.
- LanceDB rows are eventually-consistent search projections; they may lag or be incomplete and are
  never the authoritative source of body bytes or delivery state.

### 2) Delivery Index Contract

- Index keys must be deterministic and prefix-range friendly.
- Every `MessageRef` must include a stable `message_id` and enough authority metadata to reconcile it.
- `MessageRef` values must be compact and body-free; the body lives only in `cf_body`.
- Multi-key mutations for one logical write (index refs plus body) must use one `WriteBatch`.
- Index writes must be idempotent; replaying the same authority row plus body produces the same keys
  and values.

### 3) Body Storage And Compression Contract

- The message body lives in `cf_body` keyed by `authority_message_id` (one body per logical message).
- Compression is engine-managed via SST block compression (`zstd`, higher-level `bottommost`); no
  application-level per-row codec is applied to `cf_body`.
- A trained zstd dictionary is out of scope for the first rollout; plain zstd only.
- A body is never inlined into a `MessageRef` or duplicated across column families.

### 4) Archive/Search Contract

- LanceDB remains the canonical archive/search backend for searchable message documents.
- The index may reference LanceDB `document_id`, but must not implement search ranking.
- Archive document ids must be deterministic so projections can be repaired after missed dual writes.
- Search APIs must continue to use the message archive abstraction.
- Because SQLite no longer holds the body, SQL `LIKE`/substring search over message bodies is no
  longer available; body search is exclusively LanceDB's responsibility.

### 5) Sort And Cursor Contract

- `sort_id` is an authority-assigned monotonic value and must preserve chronological order within one
  prefix; wall-clock timestamps must not be the sole ordering key.
- Cursor state must be actor/channel scoped, not global.
- A cursor update must not imply message acknowledgement unless the caller explicitly performs ack.
- Ack state must preserve authority references so it can be reconciled against SQLite mailbox state.

### 6) Configuration Contract

- The first rollout must be opt-in; default installations continue to use the SQLite body and read
  path.
- Configuration should distinguish:
  - archive backend (`lancedb`);
  - message backend (`sqlite` compatibility path, or `rocksdb` for index + body);
  - RocksDB path;
  - body compression level;
  - rebuild/repair mode.
- Enabling RocksDB must not require running `sqlite-zstd`.

### 7) Migration Contract

- Migration is staged and non-destructive:
  - Phase 1 (canonical write becomes dual-body): new writes put the body into both SQLite (compat) and
    `cf_body`, with the SQLite body outbox staged in the authority transaction; normal reads remain on
    SQLite; historical bodies are backfilled into `cf_body` by idempotent, resumable, prefix-scoped
    jobs.
  - Phase 2 (canonical): once `cf_body` is validated and included in backups, the SQLite body column is
    dropped; SQLite becomes metadata-only and the `message_body_outbox` becomes the only non-RocksDB
    durable home for a not-yet-confirmed body.
- Historical SQLite rows remain readable during Phase 1.
- The first implementation is opt-in with `message_body_store.enabled = true`. A durable SQLite
  checkpoint records the staged `team_conversation_messages` prefix. Earlier sentinel rows are repaired
  only after their body is found in `cf_body` or the outbox, so enabling Phase 1 never discards a body.
  A completed backfill does not re-stage later dual-written rows, while a later SQLite-only write makes
  the next enabled run resume from that exact message boundary.
- Backfill must be idempotent, resumable by prefix scope, and must never overwrite a newer live write.
- Rebuild must be safe to interrupt.
- Rollback before Phase 2 is disabling the RocksDB backend and reading the body from SQLite; no data is
  lost. After Phase 2, rollback requires restoring `cf_body` from backup, because the body no longer
  exists in SQLite.

## Validation Matrix

- Unit tests for delivery key encoding:
  - bytewise order preserves `sort_id` order;
  - group/channel/agent/run prefixes do not collide;
  - deterministic keys are stable across replay.
- Unit tests for `MessageRef` serialization:
  - preserves `archive_document_id` and authority references;
  - contains no body bytes;
  - rejects or reports malformed values.
- Body store and compression tests:
  - body round-trips through `cf_body` (write/read equality);
  - compression ratio on a representative chat corpus is measured with plain zstd and recorded.
- RocksDB backend tests:
  - open/create the `cf_index` and `cf_body` column families;
  - append one message (index refs + body) in one `WriteBatch`;
  - range scan by channel/agent/run returns body-free refs;
  - on-demand body fetch by `message_id`;
  - cursor update/read and ack update/read.
- Integration tests for dual-write:
  - SQLite authority write succeeds when RocksDB is disabled;
  - SQLite authority write still succeeds when a RocksDB write fails after commit;
  - read-repair / high-water-mark guard surfaces a freshly written message rather than dropping it;
  - repair job rebuilds missing index keys from SQLite metadata alone (no `cf_body` dependency).
- Body durability tests:
  - the body is committed to `message_body_outbox` inside the authority transaction;
  - a `cf_body` write failure after the authority commit leaves the body recoverable from the outbox,
    and the drainer retries it into `cf_body` and clears the outbox row on ack;
  - a simulated crash between the authority commit and the `cf_body` ack loses no body (outbox replay);
  - fan-out to multiple actors stores exactly one `cf_body` entry per `authority_message_id`.
- Archive/search integration tests:
  - index `archive_document_id` points to a LanceDB document written by the archive layer;
  - search still uses LanceDB and returns the same document metadata.
- Migration tests:
  - Phase 1 dual-body write puts the body in both SQLite and `cf_body`;
  - backfill is idempotent and never clobbers a newer live write;
  - Phase 2 drop leaves single-message reads correct from `cf_body`;
  - rollback before Phase 2 reads the body from SQLite without data loss.
  - fixtures include every authority table, durable outbox, and checkpoint touched by the exercised
    path, following [test-regression-guardrails.md](test-regression-guardrails.md).
- Distributed tests:
  - node-local cache preserves `source_node_id`, `target_node_id`, and `idempotency_key`;
  - rebuilding a node-local index from main authority rows plus body restores the same delivery window.
- Compatibility tests:
  - default config uses the SQLite body and read path;
  - opt-in RocksDB config uses the index read path and `cf_body`;
  - disabling RocksDB before Phase 2 falls back to SQLite without data migration.

## Operational Notes

- RocksDB introduces a native storage dependency and must be isolated behind a feature flag or backend
  adapter boundary until CI/prebuild coverage (including the cross-compiled release targets) is stable.
- Backups must include SQLite authority data, LanceDB archive data, and the RocksDB store. After
  Phase 2 the body lives only in RocksDB, so RocksDB is no longer optional in backups.
- Operators need a debug surface to inspect key counts, cursor state, body-store size, and
  orphan/missing-body refs without dumping message bodies.
- Body compression level should be tunable. A trained dictionary is a deferred future option, not part
  of the first rollout.
- Compaction and write-buffer settings should be conservative initially; index writes are small but
  high-frequency, while body writes benefit from larger blocks and bottommost zstd.

## Open Risks

- RocksDB build and packaging may add platform-specific CI/prebuild work; this is the main cost of the
  native dependency and is amplified by the existing cross-compiled release targets.
- Body durability now depends on RocksDB backups after Phase 2, since the SQLite body column is dropped;
  weak backup/restore discipline turns a lost RocksDB into lost message bodies.
- LanceDB is eventually consistent and may be incomplete, so it is only a best-effort body-recovery
  source, not a guarantee.
- Single-message random reads decompress a whole body block; poor `block_size` tuning trades footprint
  for read amplification.
- Poor key design will make future unread, group, or actor-scoped queries expensive to add.

## Source Journals

- [docs/journal/2026-05-04-lancedb-message-archive-phase1.md](../journal/2026-05-04-lancedb-message-archive-phase1.md)
- [docs/journal/2026-05-05-message-archive-team-conversation-dual-write.md](../journal/2026-05-05-message-archive-team-conversation-dual-write.md)
- [docs/journal/2026-05-05-message-archive-team-search-api.md](../journal/2026-05-05-message-archive-team-search-api.md)
- [docs/journal/2026-05-05-message-archive-team-migration.md](../journal/2026-05-05-message-archive-team-migration.md)
- [docs/journal/2026-05-06-message-archive-step-lifecycle-run-events.md](../journal/2026-05-06-message-archive-step-lifecycle-run-events.md)
- [docs/journal/2026-05-06-task-message-correlation-authority.md](../journal/2026-05-06-task-message-correlation-authority.md)
- [docs/journal/2026-05-06-team-actor-message-group-id.md](../journal/2026-05-06-team-actor-message-group-id.md)
