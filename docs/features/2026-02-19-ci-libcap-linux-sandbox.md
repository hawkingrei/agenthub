# Linux CI Libcap Dependency For Codex Sandbox

## Background

After syncing `agenthub-codex-acp` with newer `codex` dependencies, Linux builds started failing in CI during `codex-linux-sandbox` build script execution with:

- `failed to compile vendored bubblewrap for Linux target`
- `libcap not available via pkg-config`
- missing `libcap.pc`

The dependency chain remains intentionally unchanged (`codex-arg0` + `codex-mcp-server` kept), so the fix is in CI runtime environment instead of Cargo dependency pruning.

## Scope

- Update Ubuntu job system packages in:
  - `.github/workflows/rust.yml`
  - `.github/workflows/clippy.yml`
  - `.github/workflows/bazel.yml`
- Add `libcap-dev` to existing apt install commands.
- Keep `agenthub-codex-acp` dependency graph and `arg0_dispatch_or_else` entry behavior unchanged.

## Key Decisions

- Preserve upstream-compatible `codex` entrypoint behavior (`arg0_dispatch_or_else`) and helper binary dispatch semantics.
- Solve build breakage via environment provisioning (`libcap-dev`) because the failure is from missing system `pkg-config` metadata (`libcap.pc`) on CI runners.
- Apply consistently across Rust/Cargo and Bazel workflows to avoid split-brain CI behavior.

## Validation

Run and verify on both `push` and `pull_request` events:

1. `Rust` workflow (`cargo check --workspace --locked` path)
2. `Clippy` workflow (`cargo clippy --workspace --all-targets -- -D warnings`)
3. `Bazel` workflow (`bazel build //...` and `bazel test //...`)

Record workflow run IDs in PR description before marking the TODO item as done.
