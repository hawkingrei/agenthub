# Message Archive And LanceDB

## Problem

AgentHub currently persists message-like data across several SQLite tables:

- `agent_events`
- `team_conversation_messages`
- `team_run_events`
- `team_actor_messages`

This keeps transactional writes simple, but it leaves message retrieval fragmented and makes richer
search awkward. It also stores ACP output at the raw event level, even when multiple ACP chunk
events semantically belong to one logical message.

We want a message archive layer that:

- centralizes message-shaped storage for retrieval and search
- supports LanceDB as the first archive backend
- keeps the archive backend pluggable so other databases can be added later
- aggregates ACP chunk events into logical messages before archive indexing
- supports migration of existing SQLite-resident message history into the archive

## Scope

- A backend-agnostic message archive abstraction for AgentHub.
- A LanceDB-backed archive implementation as the first concrete backend.
- Canonical message document schema for:
  - agent output events
  - Team conversation messages
  - Team run events
  - Team actor mailbox messages
  - aggregated ACP logical messages
- Archive search contract backed by the archive backend instead of ad-hoc SQLite scans.
- One-way migration from existing SQLite message history into the archive.

## Non-Goals

- Replacing all SQLite relational state in one step.
- Moving Team/task/run relational integrity into LanceDB.
- Introducing embeddings or semantic ranking in the first rollout.
- Replacing current event-stream fan-out or mailbox ack semantics.
- Changing ACP runtime transport behavior itself.

## Architecture

### 1) Split Relational State From Archive State

The archive is a message/document store, not a full replacement for relational runtime state.

SQLite remains the short-term system of record for:

- Team/task/run relational integrity
- idempotency constraints
- mailbox ack transitions
- foreign-keyed runtime metadata

The archive becomes the canonical retrieval/search surface for message-shaped history.

This means the rollout is staged:

1. define a backend-agnostic archive interface
2. introduce LanceDB as the first implementation
3. dual-write new message-shaped records into the archive
4. migrate historical SQLite message rows into archive documents
5. switch search and history-oriented read paths to the archive
6. only then evaluate whether any SQLite message tables can be demoted or trimmed

### 2) Pluggable Backend Boundary

Business logic must not depend directly on `lancedb::Connection` or LanceDB query builders.

The canonical boundary is a message archive interface with operations such as:

- ensure archive readiness
- append or upsert message documents
- append archive batches
- search documents
- run historical migration

The first concrete backend is `lancedb`, but the interface must be narrow enough that a future
SQLite FTS, Tantivy, or remote service backend can implement the same contract.

### 3) Canonical Message Document Model

The archive stores one canonical document shape for every message-like record:

- stable `document_id`
- `source_kind`
- `source_id`
- optional logical grouping IDs:
  - `team_id`
  - `run_id`
  - `conversation_id`
  - `task_id`
  - `agent_id`
  - `session_id`
- optional `logical_message_id`
- human-searchable `body_text`
- optional structured `payload_json`
- `created_at`
- optional `event_id_from`
- optional `event_id_to`
- optional `chunk_count`

`source_kind` distinguishes raw rows and higher-level logical messages. The initial kinds are:

- `agent_event`
- `team_conversation_message`
- `team_run_event`
- `team_actor_message`
- `aggregated_acp_message`

### 4) ACP Aggregation Layer

Raw ACP event persistence stays available, but archive indexing must aggregate chunked ACP message
events before storing the logical message document.

Aggregation rules for the first rollout:

- only ACP message chunk payloads are aggregated:
  - `user_message`
  - `agent_message`
  - `agent_thought`
- grouping key:
  - `agent_id`
  - `session_id`
  - `type`
  - `message_id`
- chunk text is concatenated in ascending chunk order
- the archive document records:
  - first raw event id
  - last raw event id
  - chunk count
- non-consecutive or malformed chunk sequences are treated as split logical segments in phase 1
  instead of being force-merged across gaps
- non-chunk ACP events such as `tool_call`, `tool_call_update`, `plan`, and generic
  `session_update` remain separate archive documents

This keeps the archive closer to what humans mean by “one message” while preserving traceability
back to raw event rows.

### 5) Search Contract

Archive search is text-first in the first rollout.

The initial search target is the canonical `body_text` field, backed by LanceDB full-text search.

Search queries may filter by archive scope:

- `team_id`
- `run_id`
- `conversation_id`
- `task_id`
- `agent_id`
- `session_id`
- `source_kind`

Vector or hybrid search remains a later extension and must not leak into the first archive
contract.

### 6) Historical Migration

Migration is append-oriented and idempotent.

Historical source tables:

- main/global `agent_events`
- per-agent event databases under `AgentEventDbRouter`
- `team_conversation_messages`
- `team_run_events`
- `team_actor_messages`

Migration must:

- preserve stable `document_id` derivation from source identity
- be safe to re-run without duplicate logical documents
- preserve archive traceability to source rows
- separately build aggregated ACP logical-message documents from historical ACP event rows

## Contracts

### 1) Backend Contract

- Archive business code depends only on the message archive trait.
- Backend-specific connection types stay inside backend adapter modules.
- Backend configuration must declare an explicit backend kind.
- Unsupported backend values fail fast at config load or backend initialization time.

### 2) Document Identity Contract

- Every archive document must have a stable deterministic `document_id`.
- `document_id` must be derived from source identity, not random insertion order.
- Re-running migration or replaying dual-write must reuse the same logical identity.

Recommended identity shapes:

- `agent_event:<agent_id>:<session_id>:<event_id>`
- `team_conversation_message:<conversation_id>:<message_id>`
- `team_run_event:<run_id>:<event_id>`
- `team_actor_message:<run_id>:<message_id>`
- `aggregated_acp_message:<agent_id>:<session_id>:<message_id>:<kind>`

### 3) ACP Aggregation Contract

- Archive indexing of ACP chunks must produce one logical message document per `(session_id, type,
  message_id)` group.
- Raw ACP events are still traceable through `event_id_from` / `event_id_to`.
- Aggregation must not merge different ACP message kinds together.
- If a chunk payload is malformed or lacks a usable grouping key, it falls back to raw event
  document storage instead of guessing.
- Phase 1 implementation note: when the same grouping key reappears with a non-consecutive
  `chunk_index`, the current scaffold emits a new aggregate segment for that key instead of
  force-merging across the gap.
- Follow-up contract: before archive read paths become the only canonical ACP retrieval surface,
  tighten this behavior so one stable logical message identity cannot fan out into multiple
  aggregated archive documents for the same grouping key without an explicit malformed fallback
  rule.

### 4) Search Contract

- Archive search reads from the archive backend, not directly from the legacy SQLite message
  tables.
- Search results must return enough metadata to deep-link back to the originating Agent/Team view.
- Search ranking is backend-defined FTS ranking in the first rollout; no semantic embedding ranking
  is required.

### 5) Migration Contract

- Migration is one-way from SQLite message history into archive documents.
- Migration must be resumable and idempotent.
- New dual-written documents and migrated historical documents must share the same canonical schema.

### 6) Multi-Database Extensibility Contract

- `lancedb` is the first backend, not the permanent only backend.
- The archive interface must not expose LanceDB-only query builders or table types.
- Future backends may differ in physical indexing strategy as long as they preserve:
  - document identity
  - filter semantics
  - text-search capability
  - ACP aggregation contract

## Validation Matrix

- Focused Rust tests for ACP chunk aggregation into logical messages.
- Focused Rust tests for deterministic archive `document_id` derivation.
- Focused Rust tests for backend-agnostic archive trait behavior.
- Focused Rust integration tests for LanceDB bootstrap:
  - open database
  - ensure message table
  - add message documents
  - full-text search by `body_text`
- Migration tests for:
  - Team conversation messages
  - Team run events
  - Team actor messages
  - raw ACP event replay into aggregated archive documents
- `cargo test -p <archive crate>`
- `cargo check`
- `bazel build //...`

## Operational Notes

- Keep SQLite message tables intact until archive read paths and migration are both validated.
- Treat the archive as a search/retrieval plane first, not as a replacement for transactionally
  sensitive runtime state.
- ACP aggregation should be deterministic so historical re-indexing produces the same logical
  archive documents as live dual-write.
- Phase 1 intentionally favors deterministic split-on-gap behavior over speculative chunk repair;
  stricter single-logical-message semantics for non-consecutive chunk streams remain a follow-up
  item.

## Open Risks

- LanceDB adds a new storage/runtime dependency and a new schema-evolution surface.
- Live dual-write plus historical migration can drift if document identity rules are not stable.
- ACP chunk aggregation is only as good as upstream `message_id` fidelity; malformed or missing
  metadata needs explicit fallback handling.
- A future backend might not support the same FTS ranking semantics as LanceDB, so callers must
  rely on the abstract search contract rather than backend-specific scores.

## Source Journals

- [docs/journal/2026-05-04-lancedb-message-archive-phase1.md](../journal/2026-05-04-lancedb-message-archive-phase1.md)
