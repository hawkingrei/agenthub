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

The first live dual-write surface is Team conversation messages. They continue to commit to SQLite
first, then best-effort append a deterministic archive document. Archive append failures must be
logged and recovered by the historical migration path rather than rolling back the user-visible
conversation write.

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
  - `authority_message_id`
  - `correlation_id`
  - `group_id`
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

`logical_message_id` remains the generic non-Team logical grouping field, mainly for aggregated ACP
messages. Human-visible Team projections should prefer explicit `authority_message_id +
correlation_id` fields instead of overloading `logical_message_id`.

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
- parseable chunks with the same grouping key always produce one aggregate document; gaps in
  `chunk_index` do not create additional archive documents for the same logical identity
- non-chunk ACP events such as `tool_call`, `tool_call_update`, `plan`, and generic
  `session_update` remain separate archive documents

This keeps the archive closer to what humans mean by “one message” while preserving traceability
back to raw event rows.

### 5) Search Contract

Archive search is text-first in the first rollout.

The initial search target is the canonical `body_text` field, backed by LanceDB full-text search.

Search queries may filter by archive scope:

- `authority_message_id`
- `correlation_id`
- `group_id`
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

Team run-event archive documents use the event payload for `task_id` and `conversation_id` when
present, then fall back to the owning run `input_json`. If a run-event payload has no human text,
`body_text` falls back to the event type so operational lifecycle events remain searchable.

Team actor mailbox archive documents preserve `authority_message_id` from the mailbox payload when
present, because channel fan-out messages use that field to point back to the canonical conversation
message. Their `conversation_id` is resolved from `task_conversation_id`,
`channel_conversation_id`, then `conversation_id`. Their `agent_id` is the target actor id so
actor-scoped archive filters find messages delivered to that actor.

### 3) ACP Aggregation Contract

- Archive indexing of ACP chunks must produce one logical message document per `(session_id, type,
  message_id)` group.
- Raw ACP events are still traceable through `event_id_from` / `event_id_to`.
- Aggregation must not merge different ACP message kinds together.
- If a chunk payload is malformed or lacks a usable grouping key, it falls back to raw event
  document storage instead of guessing.
- Parseable chunks are ordered by `chunk_index` when building the aggregate body. Event id remains
  the traceability boundary through `event_id_from` / `event_id_to`.
- A non-consecutive `chunk_index` sequence is still one logical message document for that grouping
  key. The archive must not fan out multiple aggregated documents for one stable logical message
  identity unless a future explicit malformed fallback document kind is introduced.

### 4) Search Contract

- Archive search reads from the archive backend, not directly from the legacy SQLite message
  tables.
- Search results must return enough metadata to deep-link back to the originating Agent/Team view.
- Search ranking is backend-defined FTS ranking in the first rollout; no semantic embedding ranking
  is required.
- The first public read surface is Team-scoped message search at
  `GET /api/teams/:id/messages/search`. The route must:
  - authenticate the caller and enforce Team ownership before searching;
  - force `team_id` from the path, not from caller-supplied query parameters;
  - accept archive filters for `authority_message_id`, `correlation_id`, `run_id`,
    `conversation_id`, `task_id`, `agent_id`, `session_id`, and `source_kind`;
  - clamp `limit` to a bounded range so archive queries cannot request unbounded result sets;
  - return archive hit metadata needed by later UI deep links.

### 5) Migration Contract

- Migration is one-way from SQLite message history into archive documents.
- Migration must be resumable and idempotent.
- New dual-written documents and migrated historical documents must share the same canonical schema.
- Archive documents carry an optional `group_id` projection field for the future
  multi-tenant/group rollout. Current Team writes leave it empty until a live authority `group_id`
  exists; archive backends must still preserve and filter it when supplied by future sources or
  re-indexing.
- Team migration batches source rows and appends each batch immediately; `batch_size` must bound both
  source rows materialized at once and archive writes.
- Team migration excludes `shared_thread_mailbox` bootstrap runs from run-event and actor-mailbox
  archive documents because those runs are internal transport bookkeeping rather than visible Team
  messages.
- The first Team migration operator endpoint is a bounded synchronous trigger. It is suitable for
  small or manually sliced backfills, but large production backfills require a durable background job
  with persisted progress before operators can rely on retry/resume visibility.
- The first migration report counts source rows converted into canonical archive documents. It does
  not distinguish newly inserted archive rows from idempotent updates; archive backends need an
  inserted/updated write-result contract before the admin API can expose that distinction.
- Live Team actor mailbox sends dual-write created rows to the archive with the same canonical
  document identity as migration.
- Live Team run-event dual-write covers new run submissions, the public run-event append path, and
  actor mailbox run-event appends.
- Live memory-flush run events emitted through `append_run_event_tx` dual-write after the enclosing
  SQLite transaction commits so archive search does not observe rolled-back flush attempts. The
  remaining tx-heavy step lifecycle insertion paths still require follow-up consolidation before
  run-event search can be fully continuous without rerunning migration.
- Historical agent event migration replays both main/global `agent_events` rows and per-agent
  `AgentEventDbRouter` rows. Parseable ACP message chunks become `aggregated_acp_message`
  documents, while non-chunk or malformed ACP rows fall back to raw `agent_event` documents with
  the same deterministic source identity.

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
  - main/global and per-agent raw ACP event replay into aggregated archive documents
  - malformed or non-chunk ACP fallback into raw `agent_event` archive documents
- Focused Team manager test for live Team conversation message dual-write without duplicating
  archive documents on idempotent retries.
- Focused Team manager tests for live Team run-event dual-write on run submission and public
  run-event appends.
- Focused Team manager tests for memory-flush run-event dual-write after transaction commit,
  including persisted/noop and failed flush attempts.
- Focused Team API test for Team-scoped archive search:
  - route uses the archive abstraction
  - route forces path Team scope into `MessageSearchQuery`
  - response preserves archive hit metadata for deep-linking
- `cargo test -p <archive crate>`
- `cargo check`
- `bazel build //...`

## Operational Notes

- Keep SQLite message tables intact until archive read paths and migration are both validated.
- Treat the archive as a search/retrieval plane first, not as a replacement for transactionally
  sensitive runtime state.
- Live archive dual-write should not make Team conversation writes depend on LanceDB availability;
  deterministic document ids and historical migration are the recovery boundary for missed archive
  appends.
- ACP aggregation should be deterministic so historical re-indexing produces the same logical
  archive documents as live dual-write.
- ACP aggregation keeps parseable chunks with the same grouping key in one logical archive
  document. Gapped chunk indexes are not repaired, but they also must not create additional
  aggregate documents for the same logical message identity.

## Open Risks

- LanceDB adds a new storage/runtime dependency and a new schema-evolution surface.
- Live dual-write plus historical migration can drift if document identity rules are not stable.
- ACP chunk aggregation is only as good as upstream `message_id` fidelity; malformed or missing
  metadata needs explicit fallback handling.
- A future backend might not support the same FTS ranking semantics as LanceDB, so callers must
  rely on the abstract search contract rather than backend-specific scores.

## Source Journals

- [docs/journal/2026-05-04-lancedb-message-archive-phase1.md](../journal/2026-05-04-lancedb-message-archive-phase1.md)
- [docs/journal/2026-05-05-message-archive-team-conversation-dual-write.md](../journal/2026-05-05-message-archive-team-conversation-dual-write.md)
- [docs/journal/2026-05-05-message-archive-team-search-api.md](../journal/2026-05-05-message-archive-team-search-api.md)
- [docs/journal/2026-05-05-message-archive-team-migration.md](../journal/2026-05-05-message-archive-team-migration.md)
- [docs/journal/2026-05-06-message-archive-group-id-projection.md](../journal/2026-05-06-message-archive-group-id-projection.md)
