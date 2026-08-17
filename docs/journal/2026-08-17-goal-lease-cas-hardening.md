# Summary

A code-only review of the "agent team" subsystem (Task/Run/Step lifecycle, goal-lease/fork
concurrency, mailbox, remote relay, permission review, actor protocol) found three related
concurrency-correctness gaps around Task/Team goal leases:

1. `task_updates.rs`'s `UPDATE team_tasks` had no optimistic-concurrency guard at all -- a decision
   computed from a stale read could be committed unconditionally, including a terminal-status write
   racing a concurrent handoff.
2. `release_task_goal_in_tx` released whatever goal lease was currently active for a task, without
   checking that it was the same lease generation the caller observed.
3. `claim_task_goal_in_tx` and `claim_execution_entity` decided "is this claimable?" in Rust from a
   plain `SELECT`, then performed an unconditional `INSERT ... ON CONFLICT DO UPDATE` -- the write
   itself didn't re-verify the precondition, so a second transaction that read the same stale snapshot
   could silently overwrite a lease/claim a concurrent transaction had already taken.

# Background

`docs/features/control-store.md`'s `ControlStore` primitives (`require_guarded_write_applied`,
`next_fencing_generation`) already exist specifically so authority writes don't hand-roll racy
check-then-act logic, and `teamspace.rs` already uses them correctly in several places
(`revoke_teamspace_member`, `accept_teamspace_invite_by_id`, `complete_goal_fork`,
`handoff_task_execution`'s own guard). `task_updates.rs` and the two claim functions in `teamspace.rs`
predate consistent adoption of that discipline.

# Scope

- `src/team/manager/task_updates.rs`: `execute_prepared_task_update`'s `UPDATE team_tasks` now includes
  `AND updated_at = <the value read when the caller's patch was computed>`, checked via
  `require_guarded_write_applied`. `release_task_goal_in_tx` is now called with the lease generation
  actually observed inside the same transaction (a fresh `SELECT lease_generation ... WHERE released_at
  IS NULL`, not whatever the eventual release statement happens to match).
- `src/team/manager/teamspace.rs`: `claim_task_goal_in_tx` and `claim_execution_entity`'s
  `ON CONFLICT DO UPDATE` clauses now carry a `WHERE` re-checking the exact same precondition the
  Rust-level pre-check already validated, but against the row as it actually is at write time -- SQLite
  skips the `DO UPDATE` (reporting 0 rows affected) when that's false, which the code now treats as a
  claim failure instead of assuming success. `handoff_task_execution`'s own `UPDATE team_tasks` and
  `release_task_goal_in_tx` also gained the generation-aware release call.
- `src/team/manager/teamspace.rs` and `src/team/manager/run_task_status_sync.rs`: their `UPDATE
  team_tasks` statements now write `updated_at = MAX(updated_at + 1, now())` instead of a plain `now()`
  (see Key Decisions).

# Key Decisions

- **`updated_at` as the CAS token, not a new schema column.** `TeamTaskRecord` already carries
  `updated_at`, bumped on every write; reusing it avoids a migration. This surfaced a real bug during
  testing: `now()` is second-granularity, so two guarded writes to the same task within the same
  wall-clock second can compute the *same* "new" value, which a third writer's `WHERE updated_at = <old
  value>` guard can't distinguish from "nothing changed." Fixed by writing `updated_at = MAX(updated_at +
  1, now())` everywhere `team_tasks.updated_at` is written (`task_updates.rs`, `teamspace.rs`'s handoff,
  `run_task_status_sync.rs`) so it's strictly monotonic per row regardless of which function writes it,
  not just self-consistent within one function. This is a real risk in production too, not just under a
  tight test loop: several of these paths are cheap, single-round-trip writes that a busy team can easily
  trigger twice within the same second (e.g. a step completing and syncing its linked task's status while
  a human concurrently hands off the task).
- **Empty-list-style permissiveness doesn't apply here; use the existing `ControlStore` idiom.** All
  three fixes follow the pattern already established in this same file (`require_guarded_write_applied`,
  guarded `UPDATE ... WHERE <precondition>`) rather than introducing a new mechanism.
- **Did not add CAS-level protection to the `TEAM_ACTIVE_GOAL_LIMIT`/`MEMBER_ACTIVE_GOAL_LIMIT`/
  `TEAM_ACTIVE_FORK_LIMIT` capacity counts** (`claim_task_goal_in_tx`'s `COUNT(*)` checks, and the
  pre-existing fork capacity check) -- these remain check-then-insert and can be oversubscribed by a
  small margin under concurrent claims for *different* tasks. Confirmed with the person requesting this
  fix as an accepted, lower-severity accounting-overshoot tradeoff (no ownership is clobbered, unlike the
  bugs this change fixes), tracked as a follow-up rather than fixed here.
- **Did not add a full CAS guard to `sync_linked_task_status_tx`** (`run_task_status_sync.rs`) -- only
  made its `updated_at` write monotonic, since a stray `now()` there was enough to defeat
  `task_updates.rs`'s guard even without that function having its own race. Adding real CAS/generation
  fencing to run-lifecycle-driven task syncs is a separate, larger scope than this review's finding.

# Validation

- `cargo test --lib team::manager::tests::task_cases` -- 14 passed, including a new
  `concurrent_terminal_status_update_and_handoff_do_not_both_apply` regression test.
- The new test uses a real WAL-backed, multi-connection SQLite pool
  (`setup_concurrent_teamspace_db`, mirroring the existing `setup_concurrent_conversation_db` pattern,
  since the shared `:memory:` pool used elsewhere is single-connection and cannot exercise genuine
  interleaving) and races `update_task_status(Completed)` against `handoff_task_execution` for the same
  task, asserting exactly one wins and the goal lease ends up released with the correct reason either
  way. A single-attempt version of this test only reproduces the vulnerable interleaving on roughly 1 in
  20 runs (`update_task_status` reaches its write in fewer awaits than `handoff_task_execution`, so it
  usually wins outright rather than losing to a stale read) -- the shipped test loops 60 fresh task/lease
  pairs per run to make detection reliable, and was confirmed to reliably fail (not just occasionally) when
  each of the three fixes was individually reverted during development.
- `cargo test --lib team::manager::` -- 184 passed, no regressions.
- `cargo test --lib` -- 767 passed; the 2 pre-existing `state::tests::*` failures
  (`lance-namespace-impls` panic, unrelated to this change) were already present on `main` before this
  change (confirmed via `git stash` in an earlier session).
- `cargo clippy --lib --tests -p agenthub` and `cargo fmt -p agenthub -- --check` clean.

# Follow-Ups

- Goal/fork capacity accounting (`TEAM_ACTIVE_GOAL_LIMIT`, `MEMBER_ACTIVE_GOAL_LIMIT`,
  `TEAM_ACTIVE_FORK_LIMIT`) remains check-then-insert and can be oversubscribed by a small margin under
  concurrent claims for different tasks.
- The other findings from the same "agent team" review round remain open, tracked in
  `docs/todo.md`'s new Agent Team Correctness item.
