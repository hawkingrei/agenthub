# Codex ACP Protocol Sync

## Background
AgentHub's Codex ACP adapter needs to track protocol updates from the upstream codex-acp integration.
The current changes align session listing and tool call payload handling with the latest ACP types.

## Scope
- Use rollout summary fields (`thread_id`, `cwd`, `first_user_message`, `updated_at`) when listing sessions.
- Remove `mcp-types` usage and rely on `codex_protocol::mcp` for tool call results and request IDs.
- Deserialize tool call content blocks directly into ACP `ContentBlock` values.
- Emit full tool call outputs for `FunctionCallOutput` events.
- Handle new remote skill events without spurious warnings.

## Key Decisions
- Trust rollout summary fields instead of parsing `SessionMetaLine` from head items.
- Treat tool call content as JSON-encoded ACP blocks, not bespoke conversions.
- Remove the `mcp-types` dependency to stay aligned with upstream codex.

## Validation
- Run `cargo check -p agenthub-codex-acp`.
- Run `cargo test -p agenthub-codex-acp`.
- Update `Cargo.lock` after refreshing codex git dependencies (`cargo update -p codex-protocol`).
