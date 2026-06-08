# Rust 1.96 Baseline

## Summary

Raised the AgentHub Rust baseline from `1.95.0` to `1.96.0`.

## Background

The repository keeps Rust pinned in both Cargo/rustup-facing files and Bazel CI
configuration. Keeping those pins aligned avoids local/CI drift and keeps Bazel
as a viable build path alongside normal Rust workflows.

## Scope

- Root `rust-toolchain.toml`
- `agenthub-codex-acp/rust-toolchain.toml`
- Bazel Rust toolchain version in `MODULE.bazel`
- GitHub Actions Rust setup pins
- Developer and user-facing baseline documentation
- Focused test assertion cleanup using Rust 1.96 `assert_matches!`

## Key Decisions

1. Keep a single workspace Rust baseline: `1.96.0`.
2. Preserve historical journal entries that recorded the previous `1.95.0`
   rollout.
3. Update visible setup documentation so operators and contributors install the
   same version used by CI.
4. Use `assert_matches!` only in focused tests where the improved failure output
   is useful and does not widen production MSRV-sensitive code paths.

## Validation

```bash
rg -n -F "1.95.0" . --hidden --glob '!Cargo.lock' --glob '!.git/**'
cargo fmt --all
cargo test worker_runtime_adjust_rejects_workdir_outside_safe_paths
cargo test member_agent_lookup_maps_row_not_found_to_missing_member_agent
git diff --check
```

Only historical journal entries should still mention `1.95.0` after the text
audit.

## Follow-Ups

- Run full `cargo check` and `bazel test //...` validation in CI or a wider
  local verification pass.
