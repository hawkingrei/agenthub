# Claude ACP Provider Support

## Summary

AgentHub now treats Claude ACP as a first-class provider and ships an AgentHub-owned generic ACP
adapter binary, `agenthub-acp`. The Claude path is selected as `agenthub-acp claude`. The
implementation keeps Claude on the existing ACP boundary: AgentHub launches the configured command,
detects the provider id, applies strict FIFO prompt delivery, and lets the adapter own
Claude-specific protocol details.

## Background

Three Claude ACP command shapes are relevant:

- `agenthub-acp claude` is AgentHub's canonical distributed command for Claude. It wraps the Rust
  `claude-code-acp-rs` library and forces ACP server mode.
- `@agentclientprotocol/claude-agent-acp` exposes the `claude-agent-acp` executable and runs ACP over
  stdio by default as a compatibility path.
- `claude-code-acp-rs` exposes the `claude-code-acp-rs` executable and requires `--acp` for ACP
  server mode as a compatibility path.

AgentHub already supported provider-neutral ACP runtimes for Codex, Gemini, and Kimi. Claude support
fits that same provider detection layer. The wrapper keeps the shipped command stable without
copying provider implementation code into AgentHub.

## Scope

- Add `crates/agenthub-acp-adapter` as the generic ACP adapter crate with an `agenthub-acp`
  binary.
- Detect `agenthub-acp claude` as Claude ACP without treating bare `agenthub-acp` as a provider.
- Detect `claude-agent-acp` as Claude ACP.
- Detect `claude-code-acp-rs --acp` as Claude ACP.
- Do not detect `claude-code-acp-rs` without `--acp`, because the binary also supports non-ACP
  modes.
- Add Claude presets and runtime labels in the web UI, with `agenthub-acp claude` as the primary
  preset.
- Document Claude ACP command setup for operators.

## Key Decisions

- Claude uses `StrictFifo` prompt delivery initially. Unlike AgentHub's Codex adapter, the Claude
  wrapper does not expose a Codex-style app-server steering contract inside AgentHub.
- Claude ignores `codex_acp.default_mode`; that setting remains scoped to AgentHub-managed Codex
  startup behavior.
- The stable AgentHub boundary remains ACP JSON-RPC over the child process stream. Claude
  credentials and model configuration stay in the adapter-supported environment or Claude settings
  files.
- The adapter depends on the published `claude-code-acp-rs` library with default features disabled,
  avoiding a build-time bundled-Claude-CLI copy step in AgentHub's default Cargo/Bazel graph.
- `agenthub-codex-acp` remains as the compatibility Codex entrypoint for this rollout. The
  follow-up `agenthub-acp codex` path is tracked in
  [2026-06-11-generic-codex-acp-entrypoint.md](2026-06-11-generic-codex-acp-entrypoint.md).

## Validation

Focused checks:

```bash
cargo test acp_provider_for_agent_requires_expected_args -- --nocapture
cargo test -p agenthub-acp-adapter
cargo check -p agenthub-acp-adapter
npm --prefix web run test -- src/agent_presets.test.ts src/components/agent_node_detail_shared.test.ts
npm --prefix web run build
npm --prefix userdocs run build
cargo fmt --check
git diff --check
```

## Follow-Ups

- Run a real Claude ACP smoke session after configuring Anthropic credentials in the deployment
  environment.
