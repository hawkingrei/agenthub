## Summary

Pinned `agenthub-codex-acp` Codex dependencies to `hawkingrei/codex@8dd355d192ddf148ab2d97c9ba89223739cfdd18` to harden remote compaction against dirty histories that contain a `CustomToolCall` without a matching `CustomToolCallOutput`.

## Why

AgentHub already repairs dirty rollout history on session resume, but live in-memory history still flows through Codex remote compaction without that repair pass. When a custom tool call is present without its output, upstream `codex-core` normalizes history by calling `error_or_panic(...)`, which panics in debug builds and aborts compaction.

The failing shape was observed locally for an `apply_patch` custom tool call recorded in a rollout session without a matching output item.

## Fork Change

Fork commit:

- `hawkingrei/codex@8dd355d192ddf148ab2d97c9ba89223739cfdd18`

Behavior change:

- `codex-core/context_manager/normalize.rs`
  - missing `CustomToolCallOutput` is now treated like a recoverable dirty-history case
  - compaction logs the issue and synthesizes an aborted `CustomToolCallOutput`
  - it no longer panics in debug builds for this case

Test change:

- upstream `history_tests.rs`
  - `normalize_adds_missing_output_for_custom_tool_call` now applies in both debug and release
  - removed the debug-only `should_panic` expectation for this case

## AgentHub Change

- `agenthub-codex-acp/Cargo.toml`
  - switched all direct Codex git dependencies from `openai/codex@230dcad...` to the fork commit above

## Validation Notes

Recommended validation:

```bash
cargo test -p agenthub-codex-acp repair_initial_history_inserts_missing_custom_tool_outputs -- --nocapture
cargo check -p agenthub-codex-acp
```

Operational validation:

- resume or compact a session that previously contained a dirty `CustomToolCall` history gap
- confirm compaction no longer panics and the session remains usable

## Follow-up

- Root-cause why `apply_patch` was recorded without a matching `CustomToolCallOutput`
- Drop the fork pin once upstream Codex carries an equivalent fix
