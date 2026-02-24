# Codex ACP Dependency Stability: Pin Codex Git Revision

## Summary

Pin `agenthub-codex-acp` codex git dependencies to a fixed commit:

- `c34b30a3c128bb75fcec27ef838c93c99b92fc61`

This replaces `branch = "acp"` to prevent upstream branch drift from breaking
local ACP adapter builds unexpectedly.

## Scope

- `agenthub-codex-acp/Cargo.toml`
- `docs/todo.md`

## Background

The codex ACP adapter had already been migrated to APIs matching codex commit
`c34b30a...`. Keeping dependencies on `branch = "acp"` makes future `cargo
update` operations pull newer upstream commits automatically, which can rebreak
the adapter without a controlled migration.

## Key Decisions

1. Pin all direct codex git dependencies in `agenthub-codex-acp/Cargo.toml` to
   the same tested commit:
   - `codex-apply-patch`
   - `codex-arg0`
   - `codex-core`
   - `codex-mcp-server`
   - `codex-protocol`
   - `codex-login`
   - `codex-utils-approval-presets`
   - `codex-utils-cli`
2. Use a single commit hash for all codex crates to keep protocol/core/config
   APIs consistent within one dependency graph.

## Validation

```bash
cargo check -p agenthub-codex-acp
```

Expected outcome:

- `agenthub-codex-acp` continues to compile.
- future branch head changes in `zed-industries/codex` do not affect this repo
  unless commit hash is explicitly updated.
