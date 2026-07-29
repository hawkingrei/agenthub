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
