# ACP Crate Split

## Background

ACP logic was tightly coupled to the AgentHub binary, which made it harder to reuse for remote agents and a future CLI. We need a reusable library surface without pulling in AgentHub-specific persistence or output delivery.

## Scope

- Introduce `crates/agenthub-acp-core` for config parsing, MCP/skills injection helpers, and prompt block construction.
- Introduce `crates/agenthub-acp` for ACP session runtime, permissions, and protocol event handling.
- Keep AgentHub-specific persistence/output in an adapter (`AgenthubAcpEventSink`) inside the main binary.

## Key Decisions

- The ACP runtime depends on a small `AcpEventSink` trait to decouple event storage and delivery.
- Safe paths are still enforced by AgentHub and passed into ACP session creation.
- AgentHub keeps ownership of `agent_events` persistence and broadcast delivery.

## Validation

- `cargo check -p agenthub`.
- Start an ACP agent and confirm:
  - sessions can be created/resumed;
  - ACP output events are persisted and streamed;
  - permission requests still appear and can be resolved.
