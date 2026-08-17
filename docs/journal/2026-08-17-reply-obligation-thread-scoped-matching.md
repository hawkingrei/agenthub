# Summary

A code-only review of the Team subsystem found that reply-obligation "credit" matching -- the mechanism
that decides which pending human-to-agent messages a visible reply satisfies -- only keyed on
`(agent_actor_id, human_actor_id)`, ignoring which thread or conversation a message actually belonged
to. Combined with the matching walk processing messages newest-to-oldest, this meant a reply in one
conversation could incorrectly close an unrelated, still-open obligation in a *different* conversation
with the same agent/human pair, while leaving the message the reply was actually answering marked open.

# Scope

- `src/team/manager/mailbox.rs`: `ReplyActorPairKey` gained a `thread_scope: Option<ReplyThreadScope>`
  field (`Thread(i64)` from `thread_root_message_id`, else `Conversation(String)` from
  `conversation_id`, else `None` for untagged messages), plus a `reply_thread_scope` helper to derive it.
- `src/team/manager/mailbox_reply_obligation_payloads.rs`, `mailbox_reply_obligations.rs`: all three
  `ReplyActorPairKey` constructors now populate `thread_scope` from the message's own
  `conversation_id`/`thread_root_message_id`.
- `src/team/manager/mailbox_reply_obligation_summary.rs`: credit lookup/consumption is now two-tier via
  new `consume_reply_credit`/`has_reply_credit` helpers -- an exact thread-scoped match is tried first;
  only when the *obligation* is thread-scoped and has no scoped credit available does it fall back to
  the untagged (`thread_scope: None`) pool, so an untagged reply can still satisfy a scoped obligation
  (preserving prior behavior for the common unthreaded case), but a reply that *does* declare a thread
  can never satisfy an obligation in a different one.

# Key Decisions

- **Two-tier matching, not a single stricter key.** The first implementation just added `thread_scope`
  to the key with no fallback, which broke a real, previously-working case: a plain agent reply with no
  `conversation_id`/`thread_root_message_id` of its own no longer matched a thread-scoped obligation at
  all (caught by an existing integration test, `team_run_messages_api_triage_resolves_open_reply_obligation`,
  which exercises exactly this "untagged reply closes a tagged obligation" scenario). An untagged reply
  carries no signal about *which* thread it answers, so falling back to the old pair-only pool for that
  specific ambiguous case is the correct, backward-compatible choice -- the fix only needs to stop a
  reply that *does* declare a thread from leaking into an unrelated one, not to require every reply to
  declare a thread.
- Left the LIFO-within-a-thread behavior (matching the most recently seen still-open obligation first)
  unchanged. The review's described failure was specifically about *unrelated* (different-thread)
  obligations being closed by the wrong reply; multiple concurrently-open, unanswered obligations within
  the *same* thread is a narrower, lower-severity scenario not addressed here.

# Validation

- New `reply_obligation_credit_matching_is_scoped_per_thread_not_just_actor_pair`
  (`mailbox_basic_cases.rs`): a human sends two reply-required messages to the same agent in two
  different conversations; a reply tagged to the first conversation closes only that one, leaving the
  second's obligation open. Confirmed this test fails (reproducing the original bug exactly) with
  `thread_scope` reverted to always `None`.
- `team_run_messages_api_triage_resolves_open_reply_obligation` (`api/teams/tests_core.rs`, pre-existing)
  -- broke when `thread_scope` was added without the loose-pool fallback (an untagged reply against a
  `conversation_id`-tagged obligation), now passes again with the two-tier lookup.
- `cargo test --lib team::manager::tests::mailbox_basic_cases` -- 18 passed.
- `cargo test --lib team::` -- 211 passed; `cargo test --lib` -- 766 passed, 3 pre-existing
  `state::tests::*` failures (unrelated `lance-namespace-impls` panic) confirmed present on `main`
  before this change.
- `cargo clippy --lib --tests -p agenthub` and `cargo fmt -p agenthub -- --check` clean.

# Follow-Ups

- The other findings from the same 2026-08-17 Team-subsystem review round remain open, tracked in
  `docs/todo.md`'s Agent Team Correctness item.
