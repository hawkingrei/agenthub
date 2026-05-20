# Summary

AgentHub's Codex ACP adapter now targets Codex `rust-v0.132.0`.

# Background

The repository had already moved to Codex `rust-v0.131.0`. This upgrade refreshes
both the Cargo git dependencies and the Bazel `codex_src` pin while adapting the
ACP bridge to Codex 0.132 protocol changes.

# Scope

- Updated `agenthub-codex-acp` Codex git dependencies from `rust-v0.131.0` to
  `rust-v0.132.0`.
- Updated `Cargo.lock` to the upstream `rust-v0.132.0` release commit
  `13595c36e218fcbd13df118eeadf00d4eb0e6d31`.
- Updated `MODULE.bazel` `codex_src` to the upstream `rust-v0.132.0` tag target.
- Adapted ACP bridge code for Codex 0.132 thread-goal status and image prompt
  model changes.

# Key Decisions

- Keep the ACP bridge tolerant of newly added user-message image metadata by
  ignoring unknown fields when converting Codex events into AgentHub thread
  records.
- Populate Codex image prompt items with `detail: None` until AgentHub exposes a
  richer image-detail selection contract.
- Treat the new `blocked` and `usage limited` thread-goal statuses as first-class
  status strings in the thread formatter to preserve user-visible progress
  reporting.

# Validation

```bash
cargo check -p agenthub-codex-acp
cargo check -p agenthub-codex-acp --tests
cargo test -p agenthub-codex-acp
```

# Follow-Ups

- Run the normal repository CI after this lands to cover non-ACP packages and
  Bazel integration.
