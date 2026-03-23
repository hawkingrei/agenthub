## Summary

- changed `agenthub actor inbox` to be read-only by default
- added explicit `--auto-ack` for intentional consume-on-read behavior
- fixed the shared `actor_inbox_with_auto_ack(...)` helper so `pending_count` reflects successful
  auto-acks instead of returning the stale pre-ack count

## Why

- default auto-ack made mailbox debugging fragile because a read unexpectedly mutated pending
  state
- it also hid the real separation between `inbox` and `ack`
- when auto-ack succeeded, the CLI still showed the old unread count, which was misleading

## Validation

- `cargo test -p agenthub actor_cli::tests::parse_inbox_uses_env_fallback -- --nocapture`
- `cargo test -p agenthub actor_cli::tests::parse_inbox_accepts_auto_ack_flag -- --nocapture`
- `cargo test -p agenthub actor_cli::tests::load_actor_inbox_keeps_pending_messages_read_only_by_default -- --nocapture`
- `cargo test -p agenthub actor_cli::tests::load_actor_inbox_auto_ack_consumes_pending_messages -- --nocapture`
- `cargo test -p agenthub-team-actor actor_inbox_with_auto_ack_marks_pending_as_delivered -- --nocapture`
- `cargo test -p agenthub-team-actor actor_inbox_with_auto_ack_keeps_pending_on_not_found -- --nocapture`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`
- `cargo clippy --locked -p agenthub-team-actor --all-targets -- -D warnings`
- `cargo fmt --all --check`
- `git -c core.fsmonitor=false diff --check`
