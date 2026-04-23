## Summary

Upgraded the repository Rust baseline to `1.95.0` and moved the ACP adapter from Codex `0.120.x` to the official Codex `0.121.0` release commit `d65ed92a5e440972626965d0af9a6345179783bc`.

## Why

- The repository toolchain and CI should converge on one current stable Rust baseline.
- `agenthub-codex-acp` should follow the upstream official Codex release rather than the older fork pin.
- Rust `1.95.0` provides `cfg_select!`, which is a good fit for small target-specific ACP adapter branches that previously used expression-style `#[cfg]` blocks.

## Changes

### Toolchain and CI

- Bumped root Rust toolchain to `1.95.0`.
- Bumped `agenthub-codex-acp` Rust toolchain to `1.95.0`.
- Updated GitHub Actions Rust setup steps to `1.95.0`.
- Updated Bazel Rust toolchain version in `MODULE.bazel` to `1.95.0`.

### Codex 0.121

- Switched `agenthub-codex-acp` git dependencies to the official `openai/codex` release commit `d65ed92a5e440972626965d0af9a6345179783bc`.
- Updated Bazel `codex_src` to the same upstream official release commit.
- Refreshed `Cargo.lock` to the new Codex graph and aligned ICU crates to the upstream `0.121.0` lockfile versions to avoid `temporal_rs` breakage from newer `icu_* 2.2.x` selections.

### Adapter compatibility fixes

- Updated ACP adapter request/notification handling for Codex `0.121` API changes:
  - `AbsolutePathBuf`-typed fields
  - `ThreadRealtimeTranscriptDelta` / `ThreadRealtimeTranscriptDone`
  - new `InProcessClientStartArgs.log_db`
  - new `McpServerConfig.supports_parallel_tool_calls`
  - new `ReviewDecision::TimedOut`

### Rust 1.95 cleanup

- Replaced two local target-specific `#[cfg]` expression branches with `cfg_select!`:
  - `agenthub-codex-acp/src/app_server_thread.rs`
  - `agenthub-codex-acp/src/thread.rs`

This keeps the logic expression-oriented and removes duplicated `#[cfg]` blocks without introducing another dependency such as `cfg-if`.

## Release note review

Reviewed:

- Rust `1.95.0` announcement: `https://blog.rust-lang.org/2026/04/16/Rust-1.95.0/`

Repository-relevant conclusions:

1. `cfg_select!` is immediately useful here and is now adopted in the ACP adapter.
2. `if let` guards inside `match` are readable, but there was no obvious high-value site in this change that justified mixing upgrade work with unrelated refactors.
3. Rust `1.95.0` destabilizes JSON target specs on stable. This repository already uses standard host toolchains plus Bazel-managed Rust setup, so no custom-target migration was required in this upgrade.

## Validation

Planned / executed checks for this upgrade:

```bash
cargo +nightly check -p agenthub-codex-acp
```

## Follow-up

- Validate whether AgentHub-managed MCP passthrough servers can safely advertise `supports_parallel_tool_calls = true` under Codex `0.121` before enabling it by default.
