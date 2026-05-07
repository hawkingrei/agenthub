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

## 2026-04-27 Follow-up After Upgrading To `openai/codex@rust-v0.125.0`

- The temporary `hawkingrei/codex@8dd355d192ddf148ab2d97c9ba89223739cfdd18` pin is no longer the live dependency source in `agenthub-codex-acp`; PR 430 moved the adapter back onto the official `openai/codex@rust-v0.125.0` baseline.
- PR 433 then verified the current upstream adapter surface that matters for Team/runtime prompt delivery:
  - managed Team/runtime ACP skills still materialize under `~/.agents/skills/agenthub-runtime/.../SKILL.md`
  - native Codex skill injection still keeps dynamic actor runtime context in a separate text prefix block
  - approvals, `ModelReroute`, and model preset/config-option flows remain aligned after the upstream upgrade
- This journal entry now serves as historical context for the temporary fork. The active backlog item is no longer “keep the fork pin alive”, but “verify dirty-history resume/compaction semantics on the official upstream baseline and document any remaining behavior gap versus the old fork repair”.

## 2026-05-07 Follow-up: Adapter-Side Guard On Official Codex

PR 545 keeps `agenthub-codex-acp` on the official `openai/codex` dependency and moves the custom
tool-output protection into the AgentHub adapter boundary:

- live Codex app-server `CustomToolCall` events are tracked in adapter-local pending state
- new user input, review, and compaction submissions are blocked if a custom tool call has not
  observed a matching `CustomToolCallOutput`
- `Undo` remains available as the recovery path and clears the adapter-local pending guard before
  rollback
- persisted dirty rollout history is repaired and atomically rewritten before `resume_thread`, so
  Codex core does not read the same missing-output gap from disk before AgentHub history replay

This closes the main behavior gap left by dropping the temporary fork: missing custom-tool outputs
are handled by AgentHub before Codex resume/normalization can panic, while the primary runtime
contract remains ACP instead of Codex app-server protocol.

Focused validation:

```bash
cargo fmt -p agenthub-codex-acp -- --check
CARGO_TARGET_DIR=/Users/weizhenwang/devel/opensource/agenthub/target-codex-live-guard cargo test -p agenthub-codex-acp persist_repaired_initial_history
CARGO_TARGET_DIR=/Users/weizhenwang/devel/opensource/agenthub/target-codex-live-guard cargo test -p agenthub-codex-acp custom_tool
CARGO_TARGET_DIR=/Users/weizhenwang/devel/opensource/agenthub/target-codex-live-guard cargo clippy -p agenthub-codex-acp --all-targets -- -D warnings
```

Merge and CI evidence:

- PR: https://github.com/hawkingrei/agenthub/pull/545
- Merge commit: `7adaf49feca552e38b1f7bd6f7e6544e59e2608e`
- Merged at: `2026-05-07T07:51:00Z`
- GitHub checks passed:
  - `Bazel Build`, `Bazel Test (Root)`, `Bazel Test (Crates)`, `Bazel Coverage`, and aggregate
    `Bazel Build and Test` in run `25482572604`
  - `Rust (Cargo)`, `Rust (Fmt)`, and `Rust (Proto Check)` in run `25482572609`
  - `Cargo Clippy` in run `25482572632`
  - `Distributed P2P Pipeline` in run `25482572593`
  - `Web` in run `25482572594`, `Web E2E` in run `25482572623`, `Web E2E Mobile` in run
    `25482572602`, and `User Docs` in run `25482572618`
  - Codecov `patch` at `92.90%` and Codecov `project` at `81.01%`
