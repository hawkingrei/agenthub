# Codex 138 Upgrade

## Summary

AgentHub's Codex ACP adapter now targets upstream Codex `rust-v0.138.0`.

## Background

The repository previously targeted Codex `rust-v0.137.0`. This upgrade keeps
the ACP adapter and generic `agenthub-acp codex` entrypoint aligned with the
upstream Codex release train while preserving the AgentHub ACP bridge behavior.

## Scope

- Updated all direct `agenthub-codex-acp` Codex git dependencies from
  `rust-v0.137.0` to `rust-v0.138.0`.
- Updated `crates/agenthub-acp-adapter`'s direct `codex-utils-cli` dependency to
  `rust-v0.138.0`.
- Refreshed `Cargo.lock` to the upstream `rust-v0.138.0` peeled release commit
  `c18e9f478bc940ef1ef8e1c426364c0fe3d86b73`.
- Updated `MODULE.bazel` `codex_src` to the same peeled release commit.
- Adapted the ACP bridge for Codex `0.138` protocol/API changes:
  - ignore the new turn moderation metadata events until AgentHub has a
    user-facing rendering contract for them;
  - clone reasoning-effort values where upstream no longer exposes them as
    `Copy`;
  - convert thread-setting working directories from Codex absolute paths to
    `PathBuf` for the app-server override request.

## Key Decisions

- Keep `TurnModerationMetadata` as a no-op bridge event for this upgrade. It is
  upstream moderation metadata, not an AgentHub-visible message yet.
- Keep the Bazel `codex_src` pin on the peeled release commit rather than the
  annotated tag object.
- Keep generic `agenthub-acp codex` and compatibility `agenthub-codex-acp`
  behavior unchanged; this is a dependency/API alignment slice.

## Validation

```bash
cargo fmt --check
cargo check -p agenthub-codex-acp
cargo check -p agenthub-codex-acp --tests
cargo check -p agenthub-acp-adapter
```

Local focused test attempt:

```bash
cargo test -p agenthub-acp-adapter
```

This command did not reach test execution in the local worktree because
dependency compilation filled the local filesystem and failed with
`No space left on device`. CI should provide the full test signal after the
branch is pushed.

## Follow-Ups

- Run normal repository CI for full adapter tests and Bazel integration on the
  refreshed Codex `0.138.0` lockfile.
