# Summary

Added `agenthub-message-store`, the backend-agnostic foundation crate for the message storage-tiering
design (compress chat/message bodies by moving them into RocksDB). This is the first implementation
step toward [features/message-storage-tiering.md](../features/message-storage-tiering.md); it carries
no RocksDB dependency yet.

# Background

The storage-tiering spec pivoted to "design B": message bodies move out of SQLite authority rows into a
RocksDB body store (`cf_body`) where SST block compression shrinks them, while a body-free delivery
index (`cf_index`) keeps ordered reads cheap. RocksDB is a native C++ dependency that needs separate
Bazel `crate.annotation` wiring (like `aws-lc-sys`/`v8`) plus cross-compiled release coverage, so the
spec requires isolating it behind a backend boundary. This crate establishes that boundary first.

# Scope

- New workspace crate `crates/agenthub-message-store` (Cargo + Bazel `BUILD.bazel`), registered in the
  Bazel crate test/coverage target lists.
- `ids`: `AuthorityMessageId` (canonical logical-message identity, the body key) vs `DeliveryMessageId`
  (per-delivery/fan-out id), plus `MessageKind`.
- `keys`: authority-assigned monotonic `sort_id` encoded big-endian (bytewise-order-preserving), and the
  ordered index key builders (`msg/by_channel`, `msg/by_agent`, `msg/by_run`, `inbox/by_actor`) plus the
  `body/by_message/<authority_message_id>` body key.
- `reference`: `MessageRef`, the body-free delivery index row, with `to_bytes`/`from_bytes`.
- `body_store`: `MessageBodyStore` trait keyed by `AuthorityMessageId` (one body per logical message) and
  an `InMemoryBodyStore` reference implementation.
- `outbox`: `BodyOutbox`, the SQLite-staged durability outbox — stage inside the authority transaction,
  drain into the store, clear only on durable ack, so a store-write failure or crash never loses a body.

# Key Decisions

- Key the body store by `authority_message_id`, not `message_id`, so fan-out to multiple actors stores
  exactly one body (matches the review fix on the spec).
- `sort_id` is an authority-assigned monotonic sequence, not a wall-clock timestamp, so ordering stays
  stable across authority tables and (later) nodes.
- Keep this crate free of RocksDB; the RocksDB backend implements `MessageBodyStore` in a follow-up PR
  where the native dependency and Bazel wiring are handled.
- Validate the compression premise in-crate with a test that measures zstd on a representative chat
  corpus, showing block-aggregated compression (what RocksDB SST does) beats per-message compression on
  short chat lines.

# Validation

- `cargo test -p agenthub-message-store` (10 tests): sort_id bytewise ordering, channel-key sequence
  ordering, prefix non-collision, deterministic/replayable keys, body key = authority id, `MessageRef`
  round-trip + body-free, fan-out one-body-per-message, outbox stage/drain, outbox failure replay, and
  the block-vs-per-message compression-ratio check.
- `cargo fmt -p agenthub-message-store -- --check` and `cargo clippy -p agenthub-message-store
  --all-targets` are clean.
- Bazel: `//crates/agenthub-message-store:agenthub_message_store_tests` added to `bazel.yml`
  crate-test and coverage target lists.

# Follow-Up

- Implement the RocksDB backend of `MessageBodyStore` (`cf_body` + `cf_index`) behind a feature flag,
  with SST zstd + bottommost zstd, plus the dual-body migration write path. Tracked in
  [todo.md](../todo.md) under Message Storage.

# 2026-07-29 Follow-Up: Delivery Index Projection Foundation

## Summary

Added the first `cf_index` delivery-projection foundation to `agenthub-message-store`. The crate now
has a backend-agnostic `MessageIndex` API, an in-memory implementation, and a RocksDB implementation
behind the existing opt-in `rocksdb` feature.

## Scope

- Added typed `MessageIndexProjection` writes for body-free `MessageRef` rows.
- Added direct `msg/by_id/<message_id>` lookup plus ordered channel, agent, run, and actor inbox scan
  prefixes.
- Opened RocksDB with both `cf_body` and `cf_index`.
- Added a RocksDB batch helper that writes one body plus its derived index refs in one `WriteBatch`.

## Key Decisions

- Keep `cf_index` strictly derived: the new API stores `MessageRef` rows only and never inlines body
  bytes.
- Keep normal application reads on the existing SQLite path; this slice only provides the index
  backend boundary and tests.
- Leave authority-derived rebuild/repair for a later slice, because that must integrate with SQLite
  authority tables rather than crate-local fixtures.

## Validation

- `cargo test -p agenthub-message-store`
- `cargo test -p agenthub-message-store --features rocksdb cf_index -- --nocapture`
- `cargo test -p agenthub-message-store --features rocksdb body_and_index_write_share_one_batch -- --nocapture`

## Follow-Ups

- Wire projection derivation from SQLite authority rows.
- Add rebuild/repair and integrity checks that rebuild `cf_index` from SQLite metadata alone.
- Keep normal reads on SQLite until ordered index reads and backup/restore evidence are reviewed.

# 2026-07-29 Follow-Up: Projection Integrity And Replay

## Summary

Added crate-local integrity and repair helpers for authority-derived `cf_index` projections. Callers
still own deriving `MessageIndexProjection` values from SQLite authority rows; the new helpers verify
that those expected refs exist in the index, report missing `cf_body` entries, and replay expected
index refs idempotently.

## Scope

- Added `MessageIndexIntegrityReport`, `MissingIndexRef`, and namespace-specific missing-ref records.
- Added `check_authority_projection_integrity(...)` for expected projection vs index/body checks.
- Added `repair_authority_projection_index(...)` to replay expected projections into the index.
- Covered both the in-memory index and the RocksDB `cf_index`/`cf_body` backend.

## Key Decisions

- Treat `cf_index` as derived and rebuildable by replaying expected projections.
- Treat missing bodies as report-only durability failures. The helper never rebuilds `cf_body` from
  index rows.
- Keep real SQLite authority derivation, orphan/prune handling, ordered read-path enablement, and
  backup validation as follow-up work.

## Validation

- `cargo test -p agenthub-message-store repair -- --nocapture`
- `cargo test -p agenthub-message-store --features rocksdb repair_report_checks_cf_index_and_cf_body -- --nocapture`

## Follow-Ups

- Derive `MessageIndexProjection` rows from SQLite authority tables.
- Add orphan detection and explicit prune mode for refs not backed by authority rows.
- Keep production reads on SQLite until high-water-mark/read-repair and backup/restore evidence land.

# 2026-08-15 Follow-Up: Rebuild And Backup-Restore Recovery Evidence

## Summary

Closed the "dual-read comparison and full rebuild/backup-restore recovery evidence" prerequisite that
[todo.md](../todo.md) requires before any control-plane authority (Team/Agent/run/mailbox/permission/
idempotency) can be considered for a future non-SQLite store. This slice only adds recovery evidence for
the already-landed `cf_body`/`cf_index` work; it does not touch control-plane authority tables, which
stay explicitly out of scope per the spec's Non-Goals.

Two integrity primitives (`check_index_refs_have_bodies`, `check_index_refs_have_authority`) and the
RocksDB checkpoint API (`create_checkpoint`) were already implemented and unit-tested in isolation, but
had zero callers wiring them into an actual loss-and-recovery scenario end to end.

## Scope

- `crates/agenthub-message-store/src/rocksdb_store.rs`:
  `checkpoint_restore_survives_source_loss_and_passes_integrity_checks` deletes the source RocksDB
  directory entirely after checkpointing (a true disaster-recovery shape, not just "the checkpoint has
  the same bytes"), then runs `check_index_refs_have_bodies` and `check_index_refs_have_authority`
  against the restored-only copy and asserts both reports are clean.
- `src/team/manager/tests/conversation_cases.rs`:
  `repair_team_conversation_message_index_recovers_full_history_after_simulated_index_loss` builds a
  healthy index, then simulates total `cf_index` loss (a fresh empty index store, standing in for a
  wiped RocksDB volume) while a message is written during the simulated outage. It drives the real,
  production `TeamManager::repair_team_conversation_message_index` rebuild path from SQLite authority
  alone, then runs `check_index_refs_have_authority` against the real SQLite-derived authority id set to
  prove the recovered index has no gaps and no orphans (the dual-read comparison), not just a matching
  row count.

## Key Decisions

- Evidence lives as executable tests, not a manual runbook: both scenarios reuse the exact production
  repair/checkpoint code paths, so the evidence stays true as the implementation evolves instead of
  rotting as prose.
- The disaster-recovery test removes the source directory before opening the restored copy, so it cannot
  pass by accidentally reading through to the original store.
- No CLI/operator surface was added in this slice. `repair_team_*_index` are `pub(crate)` methods on
  `TeamManager` inside the main binary crate; exposing them to an external operator CLI needs a public
  API decision of its own and was left out to keep this change small and reviewable.

## Validation

- `cargo test -p agenthub-message-store --features rocksdb checkpoint_restore_survives_source_loss -- --nocapture`
- `cargo test --lib team::manager::tests::conversation_cases::repair_team_conversation_message_index_recovers_full_history_after_simulated_index_loss -- --nocapture`
- `cargo test --lib team::manager::tests::conversation_cases::` (29 passed, no regressions)
- `cargo clippy -p agenthub-message-store --features rocksdb --tests` and
  `cargo clippy --lib --tests -p agenthub` are clean.

## Follow-Ups

- The remaining, larger half of the todo.md item -- defining a transactional `ControlStore` for Team/
  Agent/run/mailbox/permission/idempotency authority -- is unstarted and needs its own design spec before
  any implementation; it is not a natural extension of this slice.
- Consider promoting the RocksDB checkpoint test into the Bazel stress lane (`body-store-tests.yml`)
  rather than leaving it in the separate single-run `rust-message-store-rocksdb` cargo job.

# 2026-08-15 Follow-Up: ControlStore Design And Phase 1 Foundation

## Summary

Defined the `ControlStore` contract the todo.md item calls for
([features/control-store.md](../features/control-store.md)) and landed its Phase 1 foundation. SQLite
stays the control-plane authority -- this is not a storage-engine change, matching
[message-storage-tiering.md](../features/message-storage-tiering.md)'s existing Non-Goal that rules that
out for Team/run/permission/node/group rows. `ControlStore` is a shared, typed decision layer over
patterns already proven in production but duplicated per call site: `teamspace.rs`'s generation-fenced
CAS and guarded-completion checks, and `conversation_idempotency.rs`'s/`mailbox_queries.rs`'s
insert-then-fingerprint-compare idempotency, plus opening up the existing but narrowly-used
`team_audit_events` table to any future authority write.

## Scope

- `docs/features/control-store.md`: the stable design -- four contracts (conditional update, idempotent
  insert, audit, transaction-scoped execution), a 3-phase non-destructive migration contract (foundation
  only -> new code adopts it -> opportunistic backfill), and an explicit list of what this does *not* do
  (no engine change, no mandatory rewrite of the ~70 existing files with raw SQL against these tables).
- `crates/agenthub-db/src/control_store.rs`: the Phase 1 foundation --
  `require_guarded_write_applied`, `next_fencing_generation`, `is_unique_violation`,
  `IdempotentReplay`/`resolve_idempotent_replay`, and `record_audit_event`, all unit-tested against the
  real production schema (`init_db_at_path`), including a real SQLite `UNIQUE` violation (not a mocked
  error) and a real commit-vs-rollback audit-durability check.
- Zero existing call sites changed: `teamspace.rs`, `conversation_idempotency.rs`, `mailbox_queries.rs`,
  `manager_consts.rs`, and `src/api/teams.rs` are untouched and behave exactly as before.

## Key Decisions

- `ControlStore` centralizes *decisions*, not SQL execution: every table's INSERT/UPDATE differs in its
  columns, so threading a caller's query through a generic async closure would need boxed futures across
  an unstable HRTB boundary for little benefit. Callers keep writing their own SQL; `ControlStore`
  classifies the guard/conflict/audit outcome around it.
- Two CAS shapes only (guarded write, fencing generation) -- exactly what current production code
  already needs. A third shape is added when a real caller needs it, not designed speculatively.
- Lives in `crates/agenthub-db` (the crate that already owns the schema), not a new crate: no native
  dependency, no new Bazel wiring beyond the glob-based `rust_library` picking up the new file
  automatically.
- Zero-callers-in-Phase-1 follows the same precedent as the message-store foundation crate above: land
  the contract and its tests first, adopt incrementally, never force a mass rewrite in one PR.

## Validation

- `cargo test -p agenthub-db control_store::` (7 new tests) and `cargo test -p agenthub-db` (50 total,
  no regressions).
- `cargo clippy -p agenthub-db --all-targets` and `cargo fmt -p agenthub-db -- --check` clean.
- `cargo build -p agenthub-db -p agenthub` succeeds (the main binary crate depends on `agenthub-db`).
- `bazel test //crates/agenthub-db:agenthub_db_tests --test_arg='control_store'` (Bazel parity per
  `AGENTS.md` §2).

## Follow-Ups

- Phase 2: route new control-plane authority work (Teamspace multi-user membership writes, goal/fork
  conflict escalation, any future capability/permission table) through `ControlStore` from the start
  instead of hand-rolling a new CAS guard or unique-violation matcher.
- Phase 3 (opportunistic): backfill `teamspace.rs`'s `append_audit_event`/CAS checks,
  `conversation_idempotency.rs`'s and `mailbox_queries.rs`'s unique-violation matchers, and the
  duplicated `SQLITE_CONSTRAINT_UNIQUE_CODE` constants in `manager_consts.rs`/`src/api/teams.rs`, onto
  the shared primitives when those files are next touched for other reasons.
