# Actor Send Text-First Mailbox Contract

## Summary

- made `actor_send` prefer raw `text` for mailbox prose so markdown survives transport without being squeezed into structured fields
- kept `payload` as the compatibility path for machine-readable coordination data
- added explicit warnings when callers still use structured `payload` for `actor_send`
- extended `team_members` so actor CLI/MCP sessions can see per-member `pending_inbox_count`

## Details

- MCP `actor_send` now accepts either `text` or `payload`
- when `text` is used, the mailbox payload is stored as a raw JSON string value so markdown formatting stays intact
- when `payload` is used, MCP returns a warning in `structuredContent.warning` recommending `text` for markdown-rich messages
- CLI `agenthub actor send` now accepts `--text` in addition to `--payload-json`
- CLI sends still preserve the existing structured payload path, but print a warning to `stderr` when `--payload-json` is used
- `team_members` now includes `pending_inbox_count` for each member when a run-scoped context is available, so actors can see unread mailbox load without switching to a separate mailbox query
- Team mailbox skill examples now recommend markdown text for task briefs and status updates, and keep structured payloads only for machine-readable coordination

## Validation

- `cargo test parse_send_accepts_text_and_preserves_markdown -- --nocapture`
- `cargo test parse_send_payload_json_marks_payload_source_for_warning -- --nocapture`
- `cargo test actor_runtime_skill_includes_context_and_native_tool_contract -- --nocapture`
- `cargo test resolve_actor_send_payload_prefers_text_and_marks_payload_source -- --nocapture`
- `cargo test actor_send_returns_warning_when_structured_payload_is_used -- --nocapture`
- `cargo test describe_team_context_merges_runtime_summary_and_optional_run_overlay -- --nocapture`
- `cargo test jsonrpc_team_members_returns_live_roster_view -- --nocapture`
