# Official Codex 0.150.1 Upgrade

## Summary

AgentHub now pins the official `openai/codex` 0.150.1 release commit
`90854393966b21e9ebfd21b122334eb09a20c93d` for both Cargo and Bazel. The ACP bridge
adapts the protocol additions introduced since 0.149.1 without restoring a downstream Codex fork
or changing the two-binary runtime contract.

## Background

The previous baseline was official Codex 0.149.1 at
`ff29a44391deccde0aba0f8390337d7f3c319ea4`. The official 0.150.1 release was published on
2026-08-27 and includes the 0.150 line plus a retained-image compaction budgeting fix.

## Scope

- Align every direct Codex Cargo dependency and the Bazel `codex_src` repository on the 0.150.1
  release commit.
- Refresh the workspace lockfile for the 0.150.1 dependency graph.
- Preserve external function outputs whose new optional `call_id` is absent, while continuing to
  repair paired function and shell outputs.
- Translate the new control-agent tools, interrupted tool status, completed subagent activity, and
  command approval kind across the in-process and app-server ACP paths.
- Explicitly ignore experimental MCP event-stream and realtime timeline notifications until ACP has
  a corresponding user-facing event contract.

## Key Decisions

- Keep `openai/codex` as the only Codex source; do not reintroduce `hawkingrei/codex`.
- Pin the immutable release commit rather than the symbolic tag.
- Map interrupted collaboration calls to ACP failure state and completed subagent activity to ACP
  completion state.
- Preserve `FunctionCallOutput` entries without a `call_id`, matching upstream history semantics for
  external tool outputs.

## Validation

- `cargo check --locked -p agenthub-codex-acp-runtime -p agenthub-acp-adapter -p agenthub-daemon`
- `cargo test --locked -p agenthub-codex-acp-runtime --lib` (140 passed)
- `cargo clippy --locked -p agenthub-codex-acp-runtime -p agenthub-acp-adapter -p agenthub-daemon --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo metadata --locked --format-version 1 --no-deps` reports only the `agenthub` and
  `agenthubd` binary targets.
- `bazel mod deps --lockfile_mode=update` resolves the new Codex source without changing the module
  lockfile.
- `bazel query 'kind("rust_binary", //...)'` reports only `//:agenthub` and
  `//crates/agenthub-daemon:agenthubd`.
- Local macOS Bazel builds remain incomplete: `agenthubd` correctly rejects the Linux-only
  `vendored_bwrap_ffi` dependency on the host platform, while `agenthub` reaches the existing
  `lance-datafusion` build script and stops because the host does not provide `protoc`.

## Follow-Ups

- Let Linux CI validate both Bazel binary targets against the new `codex_src` commit.
- The advisory-range dependency versions tracked in `docs/todo.md` remain present in 0.150.1 and
  still require a future official Codex release or ordinary compatible constraints.
