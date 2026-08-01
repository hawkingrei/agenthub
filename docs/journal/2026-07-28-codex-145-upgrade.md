# Codex 145 Upgrade

## Summary

AgentHub's Codex ACP runtime now targets upstream Codex `rust-v0.145.0`.

## Background

The repository had a partial Codex upgrade where `codex-apply-patch` already
resolved to `rust-v0.145.0`, while the rest of the direct Codex crates still
resolved to `rust-v0.144.6`. The Bazel `codex_src` pin also still pointed at an
older release commit, so Cargo and Bazel were no longer aligned on the Codex
source tree.

## Scope

- Updated direct Codex git dependencies in `agenthub-codex-acp` from
  `rust-v0.144.6` to `rust-v0.145.0`.
- Updated the `agenthub-acp-adapter` `codex-utils-cli` dependency to the same
  `rust-v0.145.0` tag.
- Refreshed `Cargo.lock` to the upstream `rust-v0.145.0` peeled release commit
  `25af12f7e61572b0bc18ddb1008be543b91519b0`.
- Updated `MODULE.bazel` `codex_src` to the same peeled release commit.
- Adapted the Codex ACP runtime to upstream `0.145` protocol changes:
  - `ThreadManager::new` now receives a models manager and Codex apps tool
    cache.
  - Turn completion and abort events carry start timestamps, and turn completion
    can carry terminal error details.
  - Token usage now includes cache write input tokens.
  - MCP app context no longer carries `template_id`.
  - Web search completion can carry structured result payloads.
  - `ReviewDecision::Denied` now includes a rejection string.
  - Legacy file-system read/write roots are now represented as a struct.
  - Dynamic tool output supports audio resource items.

## Key Decisions

- Keep synthesized turn lifecycle timestamps and errors unset when AgentHub does
  not have an upstream source for them.
- Preserve app-server web search `results` when forwarding into core protocol
  events so structured out-of-band search results are not dropped.
- Ignore new environment connection and raw response completion notifications
  in the ACP bridge until AgentHub has a user-facing ACP surface for them.
- Keep the Bazel pin on the peeled release commit instead of the annotated tag
  object.

## Validation

```bash
cargo check -p agenthub-codex-acp-runtime
cargo check -p agenthub-acp-adapter
cargo test -p agenthub-codex-acp-runtime --lib
cargo test -p agenthub-acp-adapter
```

Local Bazel attempt:

```bash
bazel build //agenthub-codex-acp:agenthub_codex_acp_runtime //crates/agenthub-acp-adapter:agenthub_acp_adapter
```

This command did not enter the build graph locally because the default Bazel
configuration failed remote module initialization without Application Default
Credentials:

```text
ERROR: Failed to init auth credentials: Your default credentials were not found.
ERROR: Error initializing RemoteModule
```

## Follow-Ups

- Let CI cover the Bazel matrix for the refreshed Codex lockfile.
