# Message Index Cf Index Foundation

**Date:** 2026-07-18

## Summary

Added the first executable foundation for the RocksDB `cf_index` delivery projection. The
`agenthub-message-store` crate now has a backend-agnostic `MessageIndexStore`, an in-memory contract
implementation, authority-derived repair primitives, monotonic per-stream high-water guards, and a
RocksDB implementation that opens `cf_index` alongside `cf_body`. The first guarded read consumers
now use `cf_index` for task conversation message listing, run event timelines, cursor-based actor
inbox history, and per-agent event history only after the projection is fresh through SQLite
authority, and still hydrate records from SQLite.

## Background

The Phase 1 body rollout already keeps SQLite as the normal read path while staging bodies through a
durable SQLite outbox into RocksDB `cf_body`. The next Message Storage P1 work is the body-free
delivery index and a repair path that derives index rows from SQLite authority metadata without using
body bytes as an input.

## Scope

- Added body-free index storage and prefix-scan APIs for deterministic delivery keys.
- Added `repair_index_from_authority`, which rewrites index refs from caller-provided authority
  projections and records a repair count.
- Added `check_index_freshness`, monotonic `meta/high_water/<stream_id>` markers, and
  `repair_index_from_authority_through` so a future ordered read path can prove whether a projection
  is caught up to SQLite authority before trusting `cf_index`.
- Added RocksDB `cf_index` creation, exact read, prefix scan, and a single `WriteBatch` helper that
  writes one body plus its derived refs together.
- Added the first Team-level SQLite authority extractor for `team_conversation_messages`, compiled for
  `test`/`rocksdb` builds, which derives channel and id refs from persisted authority rows.
- Added the Team actor-mailbox SQLite authority extractor for `team_actor_messages`, which derives
  run, recipient-agent, actor-inbox, and id refs from persisted mailbox rows.
- Added the Team run-event SQLite authority extractor for `team_run_events`, which derives run and id
  refs from persisted run event rows while matching archive migration's shared-thread bootstrap
  exclusion.
- Added main and per-agent `agent_events` extractors, which derive agent, scoped run, and id refs from
  the two SQLite schemas while reusing the existing Team step/session scope resolver.
- Wired `list_task_conversation_messages` to use the configured message index as a guarded fast path:
  lagging, missing, malformed, or incomplete index state falls back to the existing SQLite query; fresh
  index rows are used only to choose ordered ids, then hydrated from SQLite.
- Wired `list_run_events` to use the same guarded fast path for the `team_run_events` run timeline.
  The run prefix can also contain actor-message refs, so the consumer filters to `team_run_events`
  before parsing delivery ids and hydrating from SQLite.
- Wired cursor-based actor inbox history pages (`include_delivered=true` with `after_id`) to the
  guarded `inbox/by_actor/<peer>:<actor>` projection. Pending-only and first-page pending-first reads
  remain on SQLite because their replacement semantics depend on message status and handling
  disposition.
- Wired local `AgentManager::list_events` and `AgentManager::list_events_for_session` to the guarded
  `msg/by_agent/<agent_id>` projection. The consumer checks `agent_events:agent:<agent_id>`
  high-water freshness and eligible ref count against the per-agent event database, preserves the
  existing newest-page then ascending-return ordering, and falls back to SQLite for missing,
  malformed, incomplete, mixed-source, or lagging projection state.
- Added a backend-agnostic index/body integrity primitive that scans selected index prefixes and
  reports refs whose `authority_message_id` is missing from the configured body store. The RocksDB
  feature tests exercise the same primitive against real `cf_index` and `cf_body` column families.
- Added a backend-agnostic orphan-ref integrity primitive that scans selected index prefixes and
  reports refs whose `authority_message_id` is not present in the caller-supplied SQLite authority id
  set. The RocksDB feature tests exercise this primitive against real `cf_index` prefix scans.
- Added exact archive document existence checks to `MessageArchiveStore` plus a backend-agnostic
  missing archive document report. The LanceDB backend checks `document_id` with a scalar filter
  instead of using full-text search.
- Updated message-store startup wiring so one RocksDB handle can be exposed as both the body store and
  the rebuildable index store without opening the same path twice.
- Kept production reads on SQLite; this change is an opt-in storage primitive, not a runtime read-path
  switch for deployments that do not enable the RocksDB-backed message store.

## Key Decisions

- `cf_index` repair remains independent of `MessageBodyStore`. Missing `cf_body` rows are integrity
  findings, not blockers for index reconstruction.
- `MessageRef` remains body-free and is serialized through the existing codec boundary.
- `msg/by_id/<message_id>` is a delivery-id lookup key, while `body/by_message/<authority_message_id>`
  remains the body key. The two identifiers stay deliberately separate to preserve fan-out semantics.
- For `team_conversation_messages`, `conversation_id` is the stable ordered channel id. The extractor
  uses the persisted group scope when present and falls back to `team_id` only for legacy rows that
  predate group propagation.
- For `team_actor_messages`, the recipient inbox id includes both peer and actor (`peer:actor`) so
  local and remote actor deliveries do not collide in the projected inbox keyspace.
- For `team_run_events`, events project only to the run timeline and id lookup keyspace for now. Agent
  and inbox projections remain owned by actor messages unless a future run-event payload contract
  promotes those fields.
- Run-event index refs use a namespaced `tre:<run_id>:<event_id>` authority id. Payload-level numeric
  `authority_message_id` remains event metadata and is not reused as a body-store key.
- Agent-event index refs use `ae:<agent_id>:<session_id>:<event_id>` as their authority/body identity
  across both main and per-agent event databases. Per-agent rows do not carry `agent_id` themselves;
  the extractor gets it from the database owner, matching archive migration.
- Main `agent_events` refs and per-agent event database refs use distinct `source_kind` values
  (`agent_events` and `per_agent_agent_events`) because they can share the `msg/by_agent/<agent_id>`
  key prefix. The local history consumer accepts only per-agent refs and compares the eligible ref
  count with SQLite authority before trusting a page.
- High-water markers are per authority stream (`team_conversation_messages`,
  `team_actor_messages`, `team_run_events`, `agent_events:main`, and `agent_events:agent:<agent_id>`)
  and are monotonic. A replayed older repair cannot move a stream backward.
- The task conversation list, run timeline, actor inbox history, and per-agent event history fast
  paths treat `cf_index` as an ordering projection only. SQLite still owns payload hydration and
  remains the fallback if the projection cannot prove freshness or completeness for the requested
  page.
- Integrity checking is diagnostic. A missing `cf_body` row is a body-loss signal for backup/outbox
  recovery, not a reason to mutate `cf_index` or treat LanceDB archive rows as authoritative.
- Orphan-ref detection is also diagnostic. The primitive reports refs that are not backed by the
  caller's authority snapshot without mutating `cf_index`.
- Orphan-ref pruning is explicit. `prune_index_refs_without_authority` uses the same caller-supplied
  authority snapshot as the diagnostic check, deletes only the exact scanned orphan keys, and leaves
  high-water markers unchanged.
- Archive-document verification is diagnostic and exact by `document_id`. Missing archive rows can be
  rebuilt from SQLite/index metadata; they are not treated as missing bodies or missing authority.
- Lagging guarded reads schedule read-repair instead of repairing inline. The scheduler records the
  affected stream, the SQLite authority maximum that must be reached, and the observed high-water
  marker, then coalesces repeated requests by keeping the highest authority bound for that stream.
  SQLite remains the served read path until a later repair pass makes the projection fresh.
- Actor inbox include-delivered first pages can use the fresh `inbox/by_actor` projection without
  weakening the pending-first rule. The read hydrates candidate ids from SQLite, checks whether any
  untriaged pending message would be hidden, and falls back to the original SQLite pending-first query
  if needed. Pending-only inbox reads stay on SQLite because status and handling disposition are
  mutable authority fields.

## Validation

```bash
cargo fmt --all
cargo test -p agenthub-message-store --locked
cargo test -p agenthub-message-store read_repair_scheduler_keeps_highest_requested_authority_bound --locked
cargo test -p agenthub-message-store explicit_orphan_prune_deletes_only_refs_without_authority --locked
cargo test -p agenthub-message-store --features rocksdb --locked
cargo test -p agenthub-message-store --features rocksdb index_authority_prune_deletes_orphan_cf_index_refs --locked
cargo test -p agenthub-message-store --features rocksdb checkpoint_restore_preserves_cf_body_cf_index_and_high_water --locked
cargo test -p agenthub-message-archive --locked
cargo test repair_team_conversation_message_index_derives_refs_from_sqlite_authority --locked
cargo test repair_team_actor_message_index_derives_refs_from_sqlite_authority --locked
cargo test repair_team_run_event_index_derives_refs_from_sqlite_authority --locked
cargo test repair_main_agent_event_index_derives_refs_from_sqlite_authority --locked
cargo test repair_per_agent_event_index_derives_refs_from_agent_event_db --locked
cargo test list_task_conversation_messages_uses_fresh_index_and_falls_back_when_lagging --locked
cargo test list_run_events_uses_fresh_index_and_falls_back_when_lagging --locked
cargo test list_actor_inbox_history_uses_fresh_index_and_falls_back_when_lagging --locked
cargo test list_actor_inbox_first_page_uses_index_without_hiding_pending --locked
cargo test list_agent_events_uses_fresh_index_and_falls_back_when_lagging --locked
cargo test list_agent_events_for_session_uses_fresh_index_with_before_cursor --locked
cargo test initialize_services_enables_push_for_main_role --locked
cargo check --features rocksdb --locked
```

The default crate test covers key ordering, body-free refs, fan-out body deduplication, outbox replay,
authority-derived index repair, body-presence integrity reports, orphan-ref integrity reports,
explicit orphan pruning, and high-water freshness checks. The targeted read-repair scheduler test
covers per-stream coalescing and non-regressing authority bounds. The targeted orphan-prune test
covers the separation between diagnostic orphan checks and explicit deletion, and proves pruning does
not mutate high-water markers. The `rocksdb` feature test covers `cf_index` prefix scans,
single-batch body-plus-ref writes, high-water persistence in `cf_index`, integrity scanning across
real `cf_index`/`cf_body` column families and caller-supplied authority ids, and exact-key orphan
deletion from the real `cf_index` column family. The checkpoint restore test proves a restored
RocksDB checkpoint can reopen with message bodies, delivery refs, and high-water markers intact.
The archive crate tests cover exact LanceDB `document_id` existence checks and the missing archive
document report.
The Team manager tests cover deriving refs from real `team_conversation_messages`,
`team_actor_messages`, `team_run_events`, main `agent_events`, and per-agent event database rows, then
marking those authority streams fresh through the repaired SQLite upper bound. The guarded read-path
tests prove lagging high-water marks do not scan the index, incomplete fresh projections fall back to
SQLite, lagging task conversation and per-agent event reads enqueue repair requests, and repaired
fresh streams use the index scan for task conversation, run event, actor inbox history, and per-agent
event pagination. The actor inbox first-page test proves a fresh index can serve history-only
include-delivered pages while falling back when the indexed page would hide pending work. Per-agent
event tests also cover SQLite tail-page ordering, session-scoped `before_id` filtering, and the count
guard needed for mixed-source agent prefixes.

## Follow-Ups
- Phase 2 body-column removal remains a separate rollout decision; it needs release-target backup
  wiring and operator recovery documentation, not just the storage-crate checkpoint proof.
