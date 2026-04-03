## Summary

- backport the narrow runtime fix from `zed-industries/codex-acp#214` into vendored `agenthub-codex-acp`
- move ACP agent-side I/O onto a dedicated OS thread/runtime instead of sharing the main `LocalSet`
- add a deterministic regression test for `apply_patch` verification over ACP-backed filesystem reads
- document the ACP runtime contract and add a verification backlog item

## Why

`agenthub-codex-acp` still ran ACP request/response progress on the same local execution lane as
tool handling. That makes `apply_patch` verification vulnerable to a deadlock when an ACP-backed
filesystem read is needed while verifying an existing file.

The fix stays narrow:

- keep the main `LocalSet` for agent/tool execution
- move `AgentSideConnection::new(...).io_task` onto a dedicated current-thread Tokio runtime
- preserve the rest of AgentHub's Codex ACP adapter behavior

## Validation

```bash
cargo fmt --manifest-path agenthub-codex-acp/Cargo.toml
cargo test -p agenthub-codex-acp apply_patch_verification_does_not_deadlock_over_acp_fs -- --nocapture
cargo test -p agenthub-codex-acp --lib
cargo clippy --locked -p agenthub-codex-acp --all-targets -- -D warnings
git diff --check
```

## Upstream

- reference: `https://github.com/zed-industries/codex-acp/pull/214`
- adapted locally instead of direct cherry-pick because AgentHub carries adapter-specific changes in `agenthub-codex-acp/src/lib.rs`
