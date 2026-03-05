# 2026-03-05 Codex ACP Upstream Sync

## Context

`agenthub-codex-acp` was behind recent `zed-industries/codex-acp` updates that improved
runtime visibility in ACP clients and compatibility of client-provided MCP server names.

## Upstream Changes Migrated

1. `678a99ec` (`src/codex_agent.rs`)
- sanitize MCP server names by replacing whitespace with `_` before inserting into config.
- applied for both `McpServer::Http` and `McpServer::Stdio`.

2. `a2784322` (`src/thread.rs`)
- forward `EventMsg::Warning` messages to ACP client stream.
- surface context compaction completion as an agent-visible status message.
- emit compaction status for both `EventMsg::ContextCompacted` and
  `EventMsg::ItemCompleted(TurnItem::ContextCompaction(..))`.

3. `34dc10c9` (`src/thread.rs`)
- handle `EventMsg::TokenCount` and send `SessionUpdate::UsageUpdate`.
- usage values map from `last_token_usage.tokens_in_context_window()` and
  `model_context_window`.

## Local Test Coverage Added

- `test_warning_forwarded_to_client`
- `test_context_compacted_event_forwarded`
- `test_token_count_emits_usage_update`
- updated `test_compact` to assert `"Context compacted"` behavior.

## Validation

- `cargo fmt --manifest-path agenthub-codex-acp/Cargo.toml`
- `cargo test --manifest-path agenthub-codex-acp/Cargo.toml`
