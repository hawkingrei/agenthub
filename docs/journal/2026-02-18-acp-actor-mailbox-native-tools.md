# ACP Actor Mailbox Native Tools

## Background

Actor runtime skill previously instructed agents to shell out through `AGENTHUB_ACTOR_CLI`.
This made mailbox coordination depend on terminal commands instead of ACP-native tool calls.

## Goal

- Expose actor mailbox operations as MCP native tools in ACP actor sessions.
- Keep existing actor mailbox semantics (`send/inbox/ack`, idempotency, states) unchanged.
- Keep CLI path as compatibility fallback, but remove it from primary runtime guidance.

## Scope

- Add an MCP stdio entrypoint in AgentHub binary: `agenthub actor-mcp`.
- Auto-inject this MCP server into ACP session setup when actor context is present.
- Update built-in actor runtime skill text to use `actor_inbox` / `actor_ack` / `actor_send`.

## Implementation

### 1. New `actor-mcp` server entrypoint

- File: `src/actor_mcp.rs`
- Reads JSON-RPC line stream from stdin/stdout.
- Implements:
  - `initialize`
  - `ping`
  - `tools/list`
  - `tools/call`
- Tools:
  - `actor_inbox`
  - `actor_ack`
  - `actor_send`
- Tool handlers call `ActorMailboxService` via `TeamManager::actor_mailbox_service()`.
- Keep bounded-idempotency behavior:
  - default deterministic idempotency key
  - explicit key validation
  - `allow_duplicate` conflict checks
  - transport/route validation (`local` vs `remote`)

### 2. ACP auto-injection of mailbox MCP server

- File: `crates/agenthub-acp/src/lib.rs`
- In actor session startup, append stdio MCP server:
  - name: `agenthub-actor-mailbox`
  - command: actor runtime binary path
  - args: `actor-mcp --run-id ... --actor-id ... --channel ...`
- Preserve existing `mcp.json` loading behavior; actor mailbox server is appended when actor context exists.

### 3. Actor runtime skill contract update

- File: `crates/agenthub-acp/src/actor_runtime_skill.rs`
- Replace CLI instruction examples with MCP native tool examples.
- Keep coordination rules unchanged (inbox-first, ack-once, idempotency discipline).

## Validation Plan

- Unit:
  - `src/actor_mcp.rs`:
    - env fallback parsing
    - exposed tool names
    - idempotency option conflict
  - `crates/agenthub-acp/src/lib.rs`:
    - injected MCP server command/args generation
  - `crates/agenthub-acp/src/actor_runtime_skill.rs`:
    - native tool contract text assertions
- Integration/manual:
  - Start an ACP agent with actor context.
  - Confirm MCP tool list includes `actor_inbox`, `actor_ack`, `actor_send`.
  - Run a send/inbox/ack loop without shelling out to `AGENTHUB_ACTOR_CLI`.

## Validation Evidence (2026-02-18)

- Command:
  - `cargo test jsonrpc_tools_list_and_call_drive_local_mailbox_flow -- --nocapture`
  - `cargo test -p agenthub-acp build_actor_mailbox_mcp_server_uses_actor_runtime_binary_and_context -- --nocapture`
  - `cargo test -p agenthub-acp actor_runtime_skill_includes_context_and_native_tool_contract -- --nocapture`
- Result:
  - passed `actor_mcp::tests::jsonrpc_tools_list_and_call_drive_local_mailbox_flow`:
    - verified JSON-RPC `tools/list` returns `actor_inbox`/`actor_ack`/`actor_send`
    - verified JSON-RPC `tools/call` executes local `send -> inbox -> ack -> inbox(include_delivered)` flow
  - passed `agenthub_acp::tests::build_actor_mailbox_mcp_server_uses_actor_runtime_binary_and_context`
  - passed `agenthub_acp::actor_runtime_skill::tests::actor_runtime_skill_includes_context_and_native_tool_contract`

## Validation Evidence (2026-02-20)

- Command:
  - `cargo test -p agenthub-acp load_mcp_servers_injects_actor_mailbox_when_config_missing -- --nocapture`
  - `cargo test -p agenthub-acp load_mcp_servers_appends_actor_mailbox_to_existing_config_servers -- --nocapture`
- Result:
  - passed `agenthub_acp::tests::load_mcp_servers_injects_actor_mailbox_when_config_missing`:
    - verifies actor runtime context auto-injects `agenthub-actor-mailbox` when `~/.agenthub/mcp.json` is absent.
  - passed `agenthub_acp::tests::load_mcp_servers_appends_actor_mailbox_to_existing_config_servers`:
    - verifies actor runtime context appends `agenthub-actor-mailbox` without dropping existing MCP servers.
  - existing `actor_mcp::tests::jsonrpc_tools_list_and_call_drive_local_mailbox_flow` continues to cover JSON-RPC `tools/list` and `tools/call` mailbox flow end-to-end.

## Risk Notes

- `actor-mcp` currently accepts JSON-RPC methods needed by Codex MCP clients; unknown methods return JSON-RPC method-not-found.
- This change does not remove CLI plumbing yet; fallback/compatibility remains.
