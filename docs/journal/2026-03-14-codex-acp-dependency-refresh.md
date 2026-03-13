# 2026-03-14 Codex ACP Dependency Refresh

## Context

`agenthub-codex-acp` still pinned codex git dependencies to
`c34b30a3c128bb75fcec27ef838c93c99b92fc61`, used
`agent-client-protocol = "=0.9.4"`, and resolved `quinn-proto = 0.11.13`.

The upstream `zed-industries/codex-acp` dependency graph moved forward to:

- `agent-client-protocol = "=0.10.2"`
- codex git dependencies resolved from branch `acp` to `ab7a5b87a9e455235f65637f1675c03051481c87`
- `quinn-proto = 0.11.14`

The goal of this change was to refresh dependencies with Cargo and keep the
local AgentHub adapter compiling against the newer codex APIs.

## Changes

### Dependency refresh

- `agenthub-codex-acp/Cargo.toml`
  - bumped package version to `0.10.0`
  - upgraded `agent-client-protocol` to `=0.10.2`
  - moved all direct codex git dependencies to
    `rev = "ab7a5b87a9e455235f65637f1675c03051481c87"`
  - added `codex-shell-command`
- `Cargo.lock`
  - refreshed via `cargo update --manifest-path agenthub-codex-acp/Cargo.toml`
  - resolved `quinn-proto` to `0.11.14`

### Compile-compat sync

Upgrading only the dependency graph was not enough: the local adapter still
targeted older codex APIs. To keep the dependency refresh buildable, the
following files were aligned to the current upstream-compatible API surface:

- `agenthub-codex-acp/src/codex_agent.rs`
- `agenthub-codex-acp/src/thread.rs`
- `agenthub-codex-acp/src/main.rs`

Scope stayed limited to codex ACP adapter compatibility and buildability. The
AgentHub-specific ACP agent identity was preserved (`agenthub-codex-acp`,
`AgentHub Codex ACP`).

## Validation

Executed locally:

- `cargo update --manifest-path agenthub-codex-acp/Cargo.toml`
- `cargo build --manifest-path agenthub-codex-acp/Cargo.toml`

Observed result:

- `agenthub-codex-acp` builds successfully against codex rev `ab7a5b87...`
  and `agent-client-protocol 0.10.2`.

## Follow-up

- run focused ACP runtime checks for session load/close, auth methods, MCP
  permission prompts, and prompt/replay flows against the refreshed adapter
- record CI evidence after the next push/PR cycle
