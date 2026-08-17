# Summary

A code-only review of the Team subsystem found that `message_index_projection.rs`'s index-repair
passes silently coerced unparseable `payload_json`/`input_json` columns to `Value::Null` (or, for
`team_run_events`, an empty `MessageArchiveScopeFallback`) with no logging. A row with genuinely
corrupt JSON -- from a bug elsewhere, manual DB surgery, or storage-layer bit rot -- would index as a
plain empty message during a rebuild, with no trace left anywhere that anything was wrong.

# Scope

- `src/team/manager/message_index_projection.rs`: added `parse_projection_json(source, message_id,
  field, raw)`, a small helper that parses a stored JSON column and, on failure, logs a structured
  `tracing::warn!` (source table, row id, field name, the parse error) before falling back to
  `Value::Null` -- same fallback behavior as before, now with a diagnostic trail. Replaced the four
  inline `serde_json::from_str(..).unwrap_or(Value::Null)` call sites (conversation messages' and actor
  messages' `payload_json`, run events' `payload_json` and the joined run's `input_json`) with calls to
  the helper.

# Key Decisions

- **Keep the fallback, add visibility.** The finding was about the silence, not the fallback choice
  itself: aborting the whole repair batch on one corrupt row would be worse (an operator can't fix a
  years-old bad row without it blocking every subsequent message from being indexed), so `Value::Null`
  stays the recovery value. The fix only makes the event observable.
- **One shared helper over four inline duplicates**, parameterized by the source table constant already
  defined for each repair function (`TEAM_CONVERSATION_MESSAGE_SOURCE`, `TEAM_ACTOR_MESSAGE_SOURCE`,
  `TEAM_RUN_EVENT_SOURCE`) and the row id already in scope, so every warning is immediately
  actionable (which table, which row, which column, why it failed) without introducing per-call-site
  logging boilerplate.

# Validation

- New `parse_projection_json_returns_parsed_value_for_valid_json` and
  `parse_projection_json_indexes_as_null_instead_of_panicking_on_corrupt_json` unit tests cover the
  helper's parsing behavior directly (valid JSON parses through; invalid JSON still yields `Value::Null`,
  preserving the pre-existing fallback contract). This codebase has no existing pattern for asserting
  `tracing` output in tests, so the tests validate the parsing/fallback behavior the refactor must not
  regress, not the log line itself.
- `cargo test -p agenthub --lib team::manager::tests::conversation_cases` (all repair-function
  integration tests) -- 29 passed.
- `cargo test -p agenthub --lib` -- 770 passed, only 2 pre-existing unrelated `state::tests::*`
  (`lance-namespace-impls`) failures.
- `cargo clippy -p agenthub --lib --tests` and `cargo fmt -p agenthub -- --check` clean.

# Follow-Ups

None outstanding from the 2026-08-17 Team-subsystem review round; all seven findings (goal-lease CAS,
permission-review reviewer-target consistency, `context_json` shape validation, client-spoofable reply
obligation suppression, thread-scoped reply-obligation credit matching, mailbox reassignment CAS/
idempotency, and this one) are now addressed.
