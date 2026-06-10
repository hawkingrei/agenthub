# Claude ACP Provider Support

## Summary

AgentHub now treats Claude ACP adapters as first-class external ACP providers without adding a
provider-specific runtime transport. The implementation keeps Claude on the existing ACP boundary:
AgentHub launches the configured command, detects the provider id, applies strict FIFO prompt
delivery, and lets the adapter own Claude-specific protocol details.

## Background

Two Claude ACP adapter shapes are relevant:

- `@agentclientprotocol/claude-agent-acp` exposes the `claude-agent-acp` executable and runs ACP over
  stdio by default.
- `claude-code-acp-rs` exposes the `claude-code-acp-rs` executable and requires `--acp` for ACP
  server mode.

AgentHub already supported provider-neutral ACP runtimes for Codex, Gemini, and Kimi. Claude support
fits that same provider detection layer; vendoring either external adapter would couple AgentHub to a
provider implementation instead of the ACP protocol surface.

## Scope

- Detect `claude-agent-acp` as Claude ACP.
- Detect `claude-code-acp-rs --acp` as Claude ACP.
- Do not detect `claude-code-acp-rs` without `--acp`, because the binary also supports non-ACP
  modes.
- Add Claude presets and runtime labels in the web UI.
- Document Claude ACP command setup for operators.

## Key Decisions

- Claude uses `StrictFifo` prompt delivery initially. Unlike AgentHub's Codex adapter, the external
  Claude adapters do not expose a Codex-style app-server steering contract inside AgentHub.
- Claude ignores `codex_acp.default_mode`; that setting remains scoped to AgentHub-managed Codex
  startup behavior.
- The stable AgentHub boundary remains ACP JSON-RPC over the child process stream. Claude adapter
  credentials and model configuration stay in the adapter-supported environment or Claude settings
  files.

## Validation

Focused checks:

```bash
cargo test acp_provider_for_agent_requires_expected_args -- --nocapture
npm --prefix web run test -- src/agent_presets.test.ts src/components/agent_node_detail_shared.test.ts
npm --prefix web run build
npm --prefix userdocs run build
cargo fmt --check
git diff --check
```

## Follow-Ups

- Run a real Claude ACP smoke session after installing one adapter and configuring Anthropic
  credentials in the deployment environment.
