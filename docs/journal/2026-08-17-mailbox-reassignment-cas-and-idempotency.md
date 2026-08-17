# Summary

A code-only review of the Team subsystem found that `reassign_reply_required_message`
(`src/team/manager/mailbox_service_escalation.rs`, backing transfer/escalate/takeover of reply-required
mailbox work) had no CAS guard on its source-message `UPDATE` -- unlike `triage_message_impl`'s
`AND handling_disposition <> ?1` pattern -- and inserted the reassigned message with
`idempotency_key = NULL`, so the existing `idx_team_actor_messages_idempotency` unique index could not
deduplicate a repeated reassignment attempt.

# Scope

- `src/team/manager/mailbox_service_escalation.rs`: the release `UPDATE` gained
  `AND handling_disposition = ?8` (bound to the disposition read at the top of the transaction) and now
  returns a `Conflict` error when it affects zero rows; the reassigned-message `INSERT` became
  `INSERT OR IGNORE` with a stable `idempotency_key` (`mailbox-reassign:{message_id}`), falling back to
  `fetch_message_by_idempotency` to resolve the pre-existing row when the insert is ignored.
- `src/team/manager/tests_support.rs`: extracted `setup_test_db`'s ~500-line inline schema into
  `create_full_test_schema(pool: &SqlitePool)` (behavior-preserving) and added
  `setup_concurrent_mailbox_db`, a WAL-mode, multi-connection variant sharing that same schema, for
  genuine-concurrency tests over the mailbox domain.
- `src/team/manager/tests/mailbox_basic_cases.rs`: two new tests (see Validation).

# Key Decisions

- **The idempotency key is the part with directly observable, fix-sensitive behavior.** A deterministic,
  single-connection test (no concurrency needed) pre-seeds a row already occupying the computed
  idempotency key and asserts `transfer_reply_required_message` resolves to that existing row instead of
  inserting a duplicate. Reverting the `idempotency_key` back to `NULL` makes this test fail immediately
  (it inserts a second, wrong row) -- this is a clean, deterministic regression test.
- **The CAS guard on the release `UPDATE` is defense-in-depth, not empirically load-bearing in this
  stack.** A genuine-concurrency test (`setup_concurrent_mailbox_db`, real WAL file, 8 connections) races
  two `transfer_reply_required_message` calls for the same source message to two different targets, 60
  times. Across 120 total race attempts (60 with the guard, 60 with it manually reverted for comparison),
  the losing side *never* hit the guard's own zero-rows branch -- it always failed earlier, either at the
  pre-existing "already terminal" read-time check (sequential timing) or with a raw SQLite
  busy/locked error surfaced as `Internal` (genuine overlap). SQLite's WAL mode already refuses to let a
  transaction commit a write built on a since-modified read snapshot, which independently prevents the
  literal duplicate-row outcome the finding worried about. The guard is kept anyway: it matches
  `triage_message_impl`'s established idiom, gives a correct `Conflict` instead of a raw database error
  in the narrow window where it *would* apply (e.g. a different journal mode, or a future refactor that
  reads the message on a separate, cached connection), and documents the invariant explicitly rather than
  relying on an implicit SQLite mechanism. Its own regression test is therefore an invariant-preserving
  concurrency test ("at most one of two concurrent transfers of the same message ever applies, and
  persisted-row count always matches the number that actually succeeded"), not a bug-reproduction test --
  it holds both before and after the guard, and that is recorded honestly rather than overclaimed.

# Validation

- New `transfer_reply_required_message_reuses_existing_row_on_idempotency_key_collision`
  (deterministic, single connection): confirmed fix-sensitive by reverting the `idempotency_key` insert
  back to `NULL` -- the test then fails because a second, wrong row gets inserted instead of the
  pre-seeded one being reused.
- New `concurrent_transfers_of_same_reply_required_message_do_not_both_apply`
  (`setup_concurrent_mailbox_db`, real WAL file, 8 connections, 60-iteration soak): races two transfers
  of the same message to different targets; asserts at most one applies and the persisted reassigned-row
  count matches the actual success count. Passes with and without the CAS guard (see Key Decisions);
  kept as ongoing invariant coverage for a function that previously had zero tests at the manager-service
  level.
- `cargo test -p agenthub --lib team::manager::tests::mailbox_basic_cases` -- 20 passed.
- `cargo test -p agenthub --lib` -- 769 passed, 2 pre-existing unrelated `state::tests::*`
  (`lance-namespace-impls`) failures, confirmed present before this change.
- `cargo clippy -p agenthub --lib --tests` and `cargo fmt -p agenthub -- --check` clean.

# Follow-Ups

- The remaining finding from the same 2026-08-17 Team-subsystem review round (silent `payload_json`
  corruption swallowing in `message_index_projection.rs`'s index repair) is tracked in the Backend
  Correctness `docs/todo.md` item.
