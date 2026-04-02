# Actor Receive And Read-Only Inbox

## Summary

- removed the shared `actor_inbox_with_auto_ack(...)` helper from the Team actor contract
- added an explicit client-side `agenthub actor receive` command for accept-and-consume mailbox flow
- kept `agenthub actor inbox` read-only for inspection/debugging
- changed HTTP `GET /api/teams/runs/:run_id/messages/inbox` back to read-only semantics
- updated managed runtime skill and Team skills to use `receive` instead of explicit routine `ack`

## Why

- mailbox acceptance belongs to the receiver/client workflow, not to `send`
- inbox reads should not mutate delivery state implicitly
- the routine agent workflow should not require a separate manual `actor ack` step
- `actor ack` still needs to exist for repair/recovery/manual compensation

## Validation

- `cargo test -p agenthub parse_receive_uses_env_fallback -- --nocapture`
- `cargo test -p agenthub load_actor_inbox_keeps_pending_messages_read_only_by_default -- --nocapture`
- `cargo test -p agenthub receive_actor_inbox_consumes_pending_messages -- --nocapture`
- `cargo test -p agenthub actor_output_preference_contract_covers_all_command_variants -- --nocapture`
- `cargo test -p agenthub teams_router_http_contract -- --nocapture`
- `cargo test -p agenthub-team-actor send_and_ack_emit_expected_events -- --nocapture`
