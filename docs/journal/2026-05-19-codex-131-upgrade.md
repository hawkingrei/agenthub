# Summary

AgentHub's Codex ACP adapter now targets Codex `rust-v0.131.0`.

# Background

The repository had already moved to Codex `rust-v0.130.0`. The next upgrade needed
to refresh both the Cargo git dependencies and the Bazel `codex_src` archive while
adapting the ACP bridge to Codex 0.131 API changes.

# Scope

- Updated `agenthub-codex-acp` Codex git dependencies from `rust-v0.130.0` to
  `rust-v0.131.0`.
- Updated `MODULE.bazel` `codex_src` to the upstream `rust-v0.131.0` tag target.
- Adapted ACP bridge code for Codex 0.131 app-server, permission, MCP, and
  environment-manager API changes.

# Key Decisions

- Use Codex's empty extension registry for AgentHub ACP sessions. AgentHub does
  not register Codex extension contributors yet, so an empty registry preserves
  the existing behavior while satisfying the new `ThreadManager` constructor.
- Reject app-server attestation requests explicitly because AgentHub ACP does not
  implement attestation generation.
- Keep app-server config loading non-strict for the embedded client to preserve
  the existing permissive adapter behavior.

# Validation

```bash
cargo check -p agenthub-codex-acp
cargo check -p agenthub-codex-acp --tests
cargo test -p agenthub-codex-acp
```

# Follow-Ups

- Run the normal repository CI after this lands to cover non-ACP packages and
  Bazel integration.
