## Summary

Upgraded `agenthub-codex-acp` from the official Codex `0.121.0` release commit `d65ed92a5e440972626965d0af9a6345179783bc` to the official `0.122.0` release commit `230dcadee609fa99d6162fe1107457030e5270a7`.

## Why

- AgentHub should stay close to the official `openai/codex` release train instead of accumulating private compatibility drift.
- The `0.122.x` line changes several app-server, protocol, config, and rollout interfaces that `agenthub-codex-acp` consumes directly.
- Recording the exact ACP-relevant delta avoids treating the full upstream release body as required follow-up for this repository.

## ACP-Relevant Upstream Delta

The upstream `0.121 -> 0.122` range is broad, but only a narrow subset is immediately relevant to AgentHub ACP:

1. MCP tool-call events now carry `mcp_app_resource_uri`.
   - `codex-rs/protocol/src/protocol.rs` adds `mcp_app_resource_uri: Option<String>` to both `McpToolCallBeginEvent` and `McpToolCallEndEvent`.
   - `codex-rs/app-server-protocol/src/protocol/v2.rs` also adds the same optional field to `ThreadItem::McpToolCall`.
   - Upstream core now derives this URI from MCP tool metadata (`ui`, `openai/outputTemplate`, or `openai/output_template`) and threads it through MCP begin/end events.

2. `apply_patch` now emits incremental structured progress.
   - `codex-rs/protocol/src/protocol.rs` adds `PatchApplyUpdated(PatchApplyUpdatedEvent)`.
   - `codex-rs/core/src/tools/handlers/apply_patch.rs` now streams parsed file-change progress while the patch input is still arriving.

3. App-server protocol adds a generic warning channel.
   - `codex-rs/app-server-protocol/src/protocol/common.rs` adds `ServerNotification::Warning`.
   - `codex-rs/app-server-protocol/src/protocol/v2.rs` defines `WarningNotification { thread_id, message }`.

4. App-server protocol adds `ExternalAgentConfigImportCompleted`.
   - This is used by upstream TUI to refresh in-memory config/plugin state after an external config import workflow.

5. MCP server config grows new defaults.
   - `codex-rs/config/src/mcp_types.rs` adds `experimental_environment` and `default_tools_approval_mode` to `McpServerConfig`.

6. Rollout thread listing now requires explicit sort direction.
   - `codex-rs/rollout/src/list.rs` adds `SortDirection`.
   - `codex-rs/rollout/src/recorder.rs` updates `RolloutRecorder::list_threads(...)` to require that direction explicitly.

## AgentHub Changes

- Updated all direct `agenthub-codex-acp` Codex git dependencies to `230dcadee609fa99d6162fe1107457030e5270a7`.
- Updated Bazel `codex_src` to the same upstream release commit.
- Refreshed `Cargo.lock` to the `0.122.0` Codex graph.
- Forwarded `mcp_app_resource_uri` through the app-server translation layer so the adapter no longer drops the new upstream field at the boundary.
- Surfaced `mcp_app_resource_uri` on ACP MCP tool calls as a `ResourceLink` content block so the app/widget target remains visible from tool-call begin through tool-call completion.
- Accepted the new generic `warning` server notification and translated it into the existing ACP warning path.
- Explicitly ignored `ExternalAgentConfigImportCompleted` in the ACP adapter because AgentHub ACP does not currently expose the upstream external-config import workflow or plugin-refresh UI.
- Added the new `McpServerConfig` fields with `None` defaults when AgentHub maps ACP-provided MCP server specs into Codex config.
- Updated session listing to pass `SortDirection::Desc` so `RolloutRecorder::list_threads(...)` keeps the existing newest-first behavior.
- Translated `PatchApplyUpdated` into an in-progress ACP `ToolCallUpdate` carrying refreshed title, locations, diff content, and raw event payload so edit tool calls no longer jump directly from begin to end.

## Behavioral Follow-Up Decision

Decision:

- Keep the `0.122` upgrade narrow, but align the ACP adapter with both new user-visible behaviors now that the required schema surface already exists.
- Do not add new ACP schema or bespoke UI affordances in this slice; reuse existing `ToolCall.content`, `ToolCall.locations`, and `ToolCallUpdate` fields.
- This closes the immediate adapter-level follow-up for `mcp_app_resource_uri` and `PatchApplyUpdated`.

## Validation

Executed:

```bash
cargo check -p agenthub-codex-acp
cargo check -p agenthub-codex-acp --tests
cargo test -p agenthub-codex-acp test_mcp_tool_call_update_preserves_mcp_app_resource_uri
cargo test -p agenthub-codex-acp test_patch_apply_updated_emits_in_progress_tool_call_update
cargo test -p agenthub-codex-acp
```

Result:

- `cargo check -p agenthub-codex-acp` passed after the compatibility updates.
- `cargo check -p agenthub-codex-acp --tests` passed after the behavior-alignment updates.
- Both focused regression tests passed:
  - `thread::tests::test_mcp_tool_call_update_preserves_mcp_app_resource_uri`
  - `thread::tests::test_patch_apply_updated_emits_in_progress_tool_call_update`
- `cargo test -p agenthub-codex-acp` passed (`87` unit tests, `0` failures).
