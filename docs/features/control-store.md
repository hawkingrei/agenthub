# Control Store

## Problem

`docs/todo.md`'s Message Storage item requires, before any control-plane authority (Team, Agent, run,
mailbox, permission, or idempotency data) can be considered for a storage change, that AgentHub "define a
transactional `ControlStore` replacement with conditional updates, uniqueness, audit, and per-entity
rollback." No such design existed; this spec is that definition.

AgentHub's control-plane authority already has each of those four properties today, but every table
reimplements them independently instead of sharing one contract:

- **Conditional updates.** `src/team/manager/teamspace.rs` already has two proven compare-and-swap (CAS)
  shapes -- a guarded `UPDATE ... WHERE <predicate>` checked against `rows_affected() == 1`
  (`team_goal_forks` completion), and a generation-fenced upsert (`SELECT` the current
  `lease_generation`, compute `generation + 1`, `INSERT ... ON CONFLICT ... DO UPDATE`, inside one
  transaction) for `team_execution_claims`. Both are correct, but neither is reusable outside
  `teamspace.rs`; a new authority table needing the same guarantee re-derives the pattern from scratch.
- **Uniqueness / idempotency.** `team_conversation_messages` and `team_actor_messages` each enforce
  idempotency with a `UNIQUE` partial index, catch the constraint violation by matching the SQLite result
  code (`2067`) and the constraint's column list against the error message, then re-fetch the existing
  row and compare a fingerprint hash to distinguish a safe replay from a real conflict. This exact
  catch/refetch/compare sequence is duplicated in `conversation_idempotency.rs` and
  `mailbox_queries.rs`, and the `2067` constant is separately redeclared in
  `src/team/manager/manager_consts.rs` and `src/api/teams.rs`.
- **Audit.** `team_audit_events` (`team_id, actor_user_id, event_kind, subject_kind, subject_id,
  detail_json, created_at`) is a general-purpose audit table, but the only code path that writes to it is
  `append_audit_event` in `teamspace.rs`, reachable only from execution-claim, goal-lease, and fork code.
  Team creation, task creation, mailbox delivery, and permission changes are not audited, not because the
  table can't represent them, but because nothing outside `teamspace.rs` calls into it.
- **Per-entity rollback.** Atomicity is already the norm, not a gap: 30+ files under `src/team` already
  wrap multi-statement authority writes in `self.db.begin() ... tx.commit()`. What's missing is a
  guarantee that CAS checks and audit writes can *only* run inside such a transaction, so a future call
  site cannot accidentally perform a guarded write or an audit append against a bare pool outside any
  rollback boundary.

Left as-is, every new authority table (Teamspace multi-user membership, goal/fork conflict escalation,
capability-based permissions) re-derives its own CAS guard, its own unique-violation matcher, and decides
audit is out of scope by default because wiring into `team_audit_events` isn't an obvious next step from
existing code. The gap is not missing mechanism -- it's a missing shared contract.

## Scope

- Define `ControlStore` as a small, typed **decision layer** over the control-plane authority tables
  already living in `crates/agenthub-db`: Team, Agent, run, mailbox, permission, node, and
  idempotency-key state.
- Cover four contracts: conditional update (CAS), idempotent insert, audit, and transaction-scoped
  execution ("per-entity rollback").
- Specify how existing call sites (`teamspace.rs`, `conversation_idempotency.rs`, `mailbox_queries.rs`,
  `manager_consts.rs`, `src/api/teams.rs`) migrate onto the shared primitives, and how new authority code
  is expected to use them from the start.

## Non-Goals

- **Not a new storage engine.** SQLite remains the control-plane authority, exactly as
  [message-storage-tiering.md](message-storage-tiering.md)'s Non-Goals already state ("Replacing Team,
  run, permission, node, or group authority rows in SQLite... SQLite remains the relational authority").
  `ControlStore` is an access-layer contract over the existing tables, not a migration target.
- **Not a mandatory rip-and-replace.** This spec does not require rewriting the 70+ existing files that
  issue raw SQL against these tables in one pass. Adoption is incremental (see Migration Contract).
- **Not a schema redesign.** Existing tables and columns are unchanged. `team_audit_events` already has
  the columns a general audit primitive needs; no new columns are required to widen its use beyond
  `teamspace.rs`.
- **Not a query builder or an ORM.** `ControlStore` does not execute table-specific SQL on a caller's
  behalf and does not thread a caller's query through generic closures. Every table's `INSERT`/`UPDATE`
  differs in its columns; forcing that through a generic async-closure API would require boxing futures
  across an unstable higher-ranked-lifetime boundary for no real benefit. Callers keep writing their own
  SQL; `ControlStore` centralizes the *decision* logic around it (was the guard satisfied? is this
  replay safe? what does an audit record look like?).
- **Not the Teamspace RBAC/capability design.** [teamspace-multi-user.md](teamspace-multi-user.md) and
  [access-control-and-roles.md](access-control-and-roles.md) own what capabilities and roles mean.
  `ControlStore` only gives that work a uniform CAS/idempotency/audit primitive to build on.
- **RocksDB indexes and LanceDB archives must not become control-plane authority by implication**
  (carried over verbatim from `docs/todo.md`): nothing in this spec changes that; `cf_index`/`cf_body`
  remain message-body/delivery-index concerns, unrelated to Team/Agent/permission authority.

## Architecture

`ControlStore` ships as `crates/agenthub-db/src/control_store.rs` (same crate that already owns
`init_db`'s schema, `message_body_outbox`, and `object_uploads`) rather than a new crate: it has no
native dependency, no Bazel wiring of its own, and belongs next to the schema it operates on.

### 1) Conditional Update (CAS)

Two named shapes, matching the two patterns already proven in `teamspace.rs`:

- **Guarded write.** Execute a caller-built `UPDATE`/`INSERT ... ON CONFLICT` whose `WHERE` clause
  encodes the precondition, then call `require_guarded_write_applied(rows_affected)`. `Ok(())` means the
  precondition held and the write applied; `Err(PreconditionFailed)` means someone else already changed,
  released, or completed the entity first. This names and types the `if updated.rows_affected() != 1 {
  anyhow::bail!(...) }` idiom already used for `team_goal_forks` completion.
- **Fencing generation.** `next_fencing_generation(current: Option<i64>) -> i64` computes the next
  generation from a value the caller already read inside the same transaction as the subsequent write
  (`None` -> `1`, `Some(g)` -> `g + 1`). This documents the exact arithmetic `claim_execution_entity`
  performs today, as an explicit, tested unit instead of inline logic -- it deliberately does not
  perform the `SELECT` itself, because the read is table-specific.

`ControlStore` does not attempt a single generic "optimistic-lock" function covering every future shape.
If a future entity needs a CAS pattern neither shape covers (e.g. a multi-row range guard), that is a
third named shape added when a real caller needs it, not designed speculatively now.

### 2) Idempotent Insert

`is_unique_violation(err: &sqlx::Error, unique_index_columns: &str) -> bool` centralizes the SQLite
`UNIQUE` constraint check: result code `2067` (`SQLITE_CONSTRAINT_UNIQUE_CODE`, exported once instead of
redeclared per file) *and* the constraint's column list appearing in the error message, so two different
unique indexes on the same table are never conflated.

`IdempotentReplay` / `resolve_idempotent_replay` centralizes the decision every current call site
reimplements after a caught violation: the caller fetches the conflicting row and computes whether its
fingerprint matches the incoming write, then calls `resolve_idempotent_replay(SamePayload |
DifferentPayload)`. `SamePayload` means a safe replay (return the existing row); `DifferentPayload` means
the idempotency key was reused for a different write (`IdempotencyConflict`).

The insert itself, the fingerprint hash, and the refetch query stay with the caller -- they are
table-specific. What moves into `ControlStore` is the classification that today lives, duplicated, in
`is_task_conversation_message_idempotency_unique_violation` and its mailbox sibling.

### 3) Audit

`record_audit_event(tx: &mut Transaction<'_, Sqlite>, event: AuditEvent<'_>) -> Result<(), sqlx::Error>`
is `teamspace.rs`'s `append_audit_event`, moved to a shared location and opened up: it takes the same
`team_id, actor_user_id, event_kind, subject_kind, subject_id, detail_json, created_at` shape the table
already has, so any future authority write can attach an audit record inside its own transaction without
adding a new table or a new code path. It requires a `&mut Transaction`, never a bare pool, so an audit
record can never be written outside the transaction boundary of the write it documents.

`team_audit_events` stays append-only and generic; this contract does not add per-domain audit tables.

### 4) Per-Entity Transaction ("Rollback")

This spec does not introduce a transaction wrapper (`self.db.begin()`/`tx.commit()` already covers this
correctly across 30+ files) -- it formalizes a rule: **every `ControlStore` primitive that mutates state
takes `&mut Transaction<'_, Sqlite>`, never a `Pool`.** `record_audit_event` already does this. Guarded
writes and idempotent inserts are executed by the caller against the same `tx` they pass into
`require_guarded_write_applied`/`resolve_idempotent_replay`. This means a failed multi-step authority
operation (e.g. create a task, claim a goal lease, and record the claim's audit event) rolls back as one
unit by construction: there is no `ControlStore` code path that could partially commit.

## Contracts

### Conditional Update Contract

- A guarded write's precondition lives entirely in the caller's `WHERE` clause; `ControlStore` only
  classifies the row-count result.
- `require_guarded_write_applied` returns `PreconditionFailed`, never panics or silently no-ops, when
  the guard did not hold.
- A fencing generation is only valid when `current` was read inside the same transaction as the write
  that consumes `next_fencing_generation`'s result; reading it outside that transaction reintroduces the
  stale-read race this contract exists to prevent.

### Idempotency Contract

- `is_unique_violation` must match on both the SQLite result code and the specific index's column list.
- Replay resolution never re-derives a fingerprint itself; the caller supplies the comparison result, so
  `ControlStore` stays agnostic of each table's fingerprint algorithm.
- A `DifferentPayload` outcome is always `IdempotencyConflict`, never silently treated as a fresh write
  or silently dropped.

### Audit Contract

- Every field on `AuditEvent` maps directly to a `team_audit_events` column; no field is optional except
  `actor_user_id` (system-initiated events have no acting user).
- `record_audit_event` must be called with the same `tx` as the write it documents, so audit and write
  are never independently observable: a crash before commit loses both, never just one.
- `detail_json` is caller-controlled, serialized via `serde_json::Value::to_string`; `ControlStore` does
  not redact or validate its contents.

### Transaction Contract

- No `ControlStore` primitive accepts a bare `SqlitePool`; every mutating function takes `&mut
  Transaction<'_, Sqlite>`.
- `ControlStore` does not manage transaction lifecycle (begin/commit/rollback stay with the caller,
  exactly as today) -- it only refuses to let a write happen outside one.

## Migration Contract

Adoption is staged and non-destructive, matching the pattern already used for
[message-storage-tiering.md](message-storage-tiering.md):

- **Phase 1 (this spec's implementation): foundation only, zero existing callers changed.**
  `crates/agenthub-db/src/control_store.rs` ships the four primitives above with unit tests against a
  real in-memory SQLite schema. `teamspace.rs`, `conversation_idempotency.rs`, `mailbox_queries.rs`,
  `manager_consts.rs`, and `src/api/teams.rs` are unchanged and keep working exactly as before.
- **Phase 2: new authority code goes through `ControlStore` from the start.** Work that introduces new
  conditional-update, idempotency, or audit needs -- Teamspace multi-user membership writes, goal/fork
  conflict escalation, future capability/permission tables -- uses these primitives instead of hand-rolling
  a new CAS guard or a new unique-violation matcher.
- **Phase 3 (opportunistic, not scheduled): backfill existing call sites.** When `teamspace.rs`,
  `conversation_idempotency.rs`, or `mailbox_queries.rs` is next touched for an unrelated reason, its
  hand-rolled CAS/idempotency/audit code can be swapped for the shared primitive as a "fix obvious local
  issues discovered during the active edit" change (per `AGENTS.md` §3), not as a dedicated mass rewrite.
  `manager_consts.rs`'s and `src/api/teams.rs`'s local `SQLITE_CONSTRAINT_UNIQUE_CODE` redeclarations are
  natural first candidates.
- **Rollback:** since Phase 1 changes no existing call site, there is nothing to roll back to. A Phase 3
  backfill of one file is reversible per-file, independent of every other file's migration state.

## Validation Matrix

- Conditional update:
  - `require_guarded_write_applied` accepts exactly `1` and rejects `0` and `2`+ with
    `PreconditionFailed`.
  - `next_fencing_generation` returns `1` for `None` and `current + 1` for `Some(current)`.
- Idempotency:
  - `is_unique_violation` returns `true` only when both the result code and the specific index's column
    list match a real `UNIQUE` violation raised against an in-memory SQLite table; returns `false` for
    unrelated errors (e.g. querying a missing table) and for a different index's column list on the same
    violation.
  - `resolve_idempotent_replay` returns `Ok(())` for `SamePayload` and `Err(IdempotencyConflict)` for
    `DifferentPayload`.
- Audit:
  - `record_audit_event` commits a row with the exact fields passed, visible after `tx.commit()`.
  - `record_audit_event` followed by `tx.rollback()` leaves zero rows in `team_audit_events` -- the audit
    record never survives independently of the transaction that produced it.
- All tests run against `crate::init_db_at_path`'s real schema (the same schema production uses), not a
  hand-built test-only table, except the two `is_unique_violation` probe tests, which use a throwaway
  probe table specifically to exercise the constraint-matching logic in isolation from any production
  table's column list.

## Operational Notes

- No new native dependency: `control_store.rs` only adds `thiserror` (already a workspace dependency
  used elsewhere) to `agenthub-db`'s `Cargo.toml`. None of the RocksDB cross-compile/glibc concerns from
  `docs/todo.md`'s Release And Packaging section apply here.
- `team_audit_events` stays append-only; this spec adds no retention, redaction, or export mechanism for
  it. If audit data grows large enough to need retention policy, that is a separate follow-up, not part
  of opening the table up to more callers.
- Bazel picks up the new module automatically (`crates/agenthub-db/BUILD.bazel`'s `rust_library` globs
  `src/**/*.rs`); the new `thiserror` dependency edge resolves against the crate already vendored for
  other workspace members, so no crate-index repin was needed.

## Open Risks

- **Adoption is opt-in, not enforced.** Nothing in Phase 1 forces existing call sites to migrate, and
  nothing mechanically prevents a new authority table from hand-rolling its own CAS/idempotency/audit
  code anyway. Without the Phase 3 habit ("touch it, then migrate it") actually happening over time, this
  can bit-rot as an unused library, the same risk any shared abstraction has with zero mandatory callers.
- **Two CAS shapes may not be exhaustive.** `guarded_update`/fencing-generation cover every current
  production case, but a future entity with a genuinely different concurrency shape (e.g. a multi-row
  range guard) is not yet designed. Adding a third shape prematurely, before a real caller needs it, risks
  guessing wrong about its actual requirements.
- **This spec does not itself resolve the larger `docs/todo.md` item.** "Before moving Team, Agent, run,
  mailbox, permission, or idempotency authority" off ad hoc per-call-site patterns is a multi-PR adoption
  effort (Phase 2/3 above), not something Phase 1 alone completes. This spec unblocks that work; it does
  not finish it.

## Source Journals

- [docs/journal/2026-08-07-team-goal-lease-foundation.md](../journal/2026-08-07-team-goal-lease-foundation.md)
  -- the generation-fenced claim pattern this spec generalizes.
- [docs/journal/2026-06-10-message-store-foundation-crate.md](../journal/2026-06-10-message-store-foundation-crate.md)
  -- the sibling foundation-crate-first, zero-callers-in-Phase-1 precedent this spec follows.
