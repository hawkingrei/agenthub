# Codex 136 Upgrade

## Summary

AgentHub's Codex ACP adapter now targets upstream Codex `rust-v0.136.0`.

## Background

The repository previously targeted Codex `rust-v0.135.0`. This upgrade keeps
the ACP adapter aligned with the upstream Codex release train while preserving
the existing AgentHub turn-start and turn-steer behavior.

## Scope

- Updated `agenthub-codex-acp` Codex git dependencies from `rust-v0.135.0` to
  `rust-v0.136.0`.
- Refreshed `Cargo.lock` to the upstream `rust-v0.136.0` release commit
  `7ca611348db9446711ed16ed81c84095e3721cee`.
- Updated `MODULE.bazel` `codex_src` to the same upstream release commit.
- Updated the shared `rmcp` lock from `1.6.0` to `1.7.0`, matching the new
  Codex app-server protocol requirement.
- Adapted ACP bridge code for Codex `0.136` protocol changes:
  - `TurnStartParams` and `TurnSteerParams` now receive
    `client_user_message_id: None`.
  - `UserMessageEvent` matching ignores the new upstream client field.

## Key Decisions

- Keep `client_user_message_id` unset until AgentHub has a durable
  client-originated message id surface that should be forwarded to Codex.
- Keep event handling non-exhaustive for user-message events so future upstream
  metadata additions do not break AgentHub when the adapter does not need the
  new field.
- Keep the Bazel `codex_src` pin on the peeled release commit rather than the
  annotated tag object.

## Validation

```bash
cargo check -p agenthub-codex-acp
cargo check -p agenthub-codex-acp --tests
```

Local full test attempt:

```bash
cargo test -p agenthub-codex-acp
```

This command did not reach test execution in the local worktree because archive
generation failed with `No space left on device`. CI should provide the full
test signal after the branch is pushed.

## Follow-Ups

- Run normal repository CI to cover full adapter tests and Bazel integration on
  the refreshed Codex `0.136.0` lockfile.
