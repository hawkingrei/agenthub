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

## Risk Notes

- `actor-mcp` currently accepts JSON-RPC methods needed by Codex MCP clients; unknown methods return JSON-RPC method-not-found.
- This change does not remove CLI plumbing yet; fallback/compatibility remains.
