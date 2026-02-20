# Linux CI Libcap Dependency For Codex Sandbox

## Background

After syncing `agenthub-codex-acp` with newer `codex` dependencies, Linux builds started failing in CI during `codex-linux-sandbox` build script execution with:

- `failed to compile vendored bubblewrap for Linux target`
- `libcap not available via pkg-config`
- missing `libcap.pc`
- Bazel runfiles error: `expected vendored bubblewrap .../../vendor/bubblewrap, but it was not found`

The dependency chain remains intentionally unchanged (`codex-arg0` + `codex-mcp-server` kept), so the fix is in CI runtime environment instead of Cargo dependency pruning.

## Scope

- Update Ubuntu job system packages in:
  - `.github/workflows/rust.yml`
  - `.github/workflows/clippy.yml`
  - `.github/workflows/bazel.yml`
- Add `libcap-dev` to existing apt install commands.
- Add Bazel mid-term override for `codex-linux-sandbox` in `MODULE.bazel`:
  - fetch `@codex_src` at lockfile-pinned codex revision;
  - disable `codex-linux-sandbox` cargo build script (`gen_build_script = "off"`);
  - inject `--cfg=vendored_bwrap_available`;
  - link `@@//third_party/codex_linux_sandbox:vendored_bwrap_ffi` (canonical main-repo label, avoids resolving to the generated external crate repo namespace).
- Add Bazel patch override for `codex-core` in `MODULE.bazel`:
  - apply `//third_party/codex_core:codex_core_node_version.patch` to rewrite `include_str!("../../../../node-version.txt")` to crate-local path;
  - add crate-local `node-version.txt` for Bazel-generated git crate mirror where workspace-root files are not present.
- Add `third_party/codex_linux_sandbox/BUILD.bazel` and `config.h` to compile vendored bubblewrap C sources from `@codex_src//codex-rs/vendor` and link system `libcap` via `-lcap`.
- Keep `agenthub-codex-acp` dependency graph and `arg0_dispatch_or_else` entry behavior unchanged.

## Key Decisions

- Preserve upstream-compatible `codex` entrypoint behavior (`arg0_dispatch_or_else`) and helper binary dispatch semantics.
- Solve build breakage via environment provisioning (`libcap-dev`) because the failure is from missing system `pkg-config` metadata (`libcap.pc`) on CI runners.
- Solve Bazel-only runfiles layout mismatch by moving Linux bubblewrap wiring into Bazel graph construction (crate annotation + `cc_library`) instead of runtime `CODEX_BWRAP_SOURCE_DIR` injection in CI scripts.
- Use canonical main-repo label (`@@//...`) for crate annotation deps so bzlmod/crate_universe does not reinterpret the path as an in-repo package under `@crate_index__codex-linux-sandbox`.
- Patch `codex-core` git crate for Bazel-only source layout mismatch (`node-version.txt` sits at codex workspace root in upstream repo, but not in crate mirror root), while keeping Cargo dependency graph unchanged.
- Apply fixes consistently across Rust/Cargo and Bazel workflows to avoid split-brain CI behavior.

## Validation

Run and verify on both `push` and `pull_request` events:

1. `Rust` workflow (`cargo check --workspace --locked` path)
2. `Clippy` workflow (`cargo clippy --workspace --all-targets -- -D warnings`)
3. `Bazel` workflow (`bazel build //...` and `bazel test //...`)

Record workflow run IDs in PR description before marking the TODO item as done.
