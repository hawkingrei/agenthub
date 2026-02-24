# ACP Tool Output Content Filtering

## Background
Tool call results may include content blocks that cannot be deserialized into ACP `ContentBlock` values. These invalid blocks are skipped to avoid crashing the client, but the behavior needs coverage to prevent regressions.

## Scope
- Factor tool output content mapping into a helper function.
- Add a unit test that keeps valid blocks and skips invalid ones.

## Key Decisions
- Preserve the existing warn-level logging when deserialization fails.
- Keep the filtering behavior consistent with current runtime handling.

## Validation
- Run `cargo test -p agenthub-codex-acp` and confirm the new unit test passes.
