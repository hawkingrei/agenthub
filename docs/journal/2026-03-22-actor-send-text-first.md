# Actor Send Text-First Mailbox Contract

## Summary

- made `actor_send` prefer raw `text` for mailbox prose so markdown survives transport without being squeezed into structured fields
- kept `payload` as the compatibility path for machine-readable coordination data
- added explicit warnings when callers still use structured `payload` for `actor_send`
- extended `team_members` so actor CLI/MCP sessions can see per-member `pending_inbox_count`
- added `channel_id` mailbox sends so actors can send directly into Team channels such as `all`
- clarified that Team channel `@member_id` stays broadcast fan-out while receivers get mention metadata
- documented human mailbox targets (`user` / `user:<id>`) as notification delivery for urgent escalation

## Details

- MCP `actor_send` now accepts either `text` or `payload`
- when `text` is used, the mailbox payload is stored as a raw JSON string value so markdown formatting stays intact
- when `payload` is used, MCP returns a warning in `structuredContent.warning` recommending `text` for markdown-rich messages
- CLI `agenthub actor send` now accepts `--text` in addition to `--payload-json`
- CLI and MCP `actor_send` now also accept `channel_id`, which persists one canonical channel message and then fans out mailbox deliveries to the relevant Team members
- CLI sends still preserve the existing structured payload path, but print a warning to `stderr` when `--payload-json` is used
- `team_members` now includes `pending_inbox_count` for each member when a run-scoped context is available, so actors can see unread mailbox load without switching to a separate mailbox query
- Team channel mailbox fan-out now preserves `mention_actor_ids` / `mentioned_actor_ids` in the forwarded payload while keeping group-chat recipients broadcast
- Direct human notifications remain `to_actor_id = user` / `user:<id>` and are intended for urgent operator-facing escalation
- Channel mailbox fan-out now auto-routes remote team members over p2p by synthesizing the
  registered gRPC relay route from the recipient agent target node, while still persisting one
  canonical conversation message on the AgentHub authority node first
- The AgentHub-side canonical conversation row is the durable persistence source of truth for
  channel sends; we do not create a duplicate mailbox mirror row just for persistence
- Remote nodes now persist a deduplicated channel replica row keyed by the authority message so
  node-local history queries can use backup/query data without changing canonical ownership
- Team mailbox skill examples now recommend markdown text for task briefs and status updates, and keep structured payloads only for machine-readable coordination
- Follow-up after merging the reporting-guidance work: shared Team skills now spell out that
  channel mailbox sends are authority-first, `@member_id` in channel text is metadata only, and
  urgent operator escalation should use the human mailbox notification path instead of inventing a
  new routing mode

## Validation

- `cargo test parse_send_accepts_text_and_preserves_markdown -- --nocapture`
- `cargo test parse_send_payload_json_marks_payload_source_for_warning -- --nocapture`
- `cargo test parse_send_accepts_channel_id_target -- --nocapture`
- `cargo test parse_send_rejects_conflicting_actor_and_channel_targets -- --nocapture`
- `cargo test actor_runtime_skill_includes_context_and_native_tool_contract -- --nocapture`
- `cargo test resolve_actor_send_payload_prefers_text_and_marks_payload_source -- --nocapture`
- `cargo test actor_send_returns_warning_when_structured_payload_is_used -- --nocapture`
- `cargo test actor_mailbox_service_channel_send_broadcasts_and_preserves_mentions -- --nocapture`
- `cargo test actor_mailbox_service_channel_send_auto_routes_remote_recipients_over_p2p -- --nocapture`
- `cargo test internal_grpc_mailbox_send_persists_channel_replica_history -- --nocapture`
- `cargo test describe_team_context_merges_runtime_summary_and_optional_run_overlay -- --nocapture`
- `cargo test jsonrpc_team_members_returns_live_roster_view -- --nocapture`
