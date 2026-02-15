# Bazel Native `rules_rust` Migration (Core Rust)

## Background

The previous Bazel integration used shell wrapper targets (`workspace_shell_build` / `workspace_shell_test`) that executed `cargo` and `npm` directly. This provided a unified entrypoint but was not a native Bazel graph and had non-hermetic behavior (workspace writes, implicit dependencies, and poor cache semantics).

## Scope

- Replace root shell-wrapper Bazel targets with native Rust targets based on `rules_rust`.
- Introduce per-crate `BUILD.bazel` files for core Rust crates used by the main AgentHub service.
- Update CI and developer commands to use `bazel build //...` and `bazel test //...`.
- Remove runtime `build.rs` proto generation for the main service crate and switch to checked-in generated protobuf Rust code to simplify Bazel integration.

## Key Decisions

1. Use `rules_rust` + `crate_universe` from `MODULE.bazel` as the dependency backbone.
2. Keep Cargo workflow intact for normal Rust development; Bazel-specific dependency graph generation is isolated via `Cargo.bazel.toml`.
3. Keep `agenthub-codex-acp` in the same `crate_universe` graph and expose native Bazel targets for its library, binary, and tests.
4. Replace `tonic::include_proto!` (OUT_DIR-based) with checked-in generated code at `src/internal/proto/agenthub.internal.v1.rs` so Bazel and Cargo both compile without dynamic proto codegen.
5. Remove obsolete shell-wrapper scripts under `bazel/ci/` after migration.

## Files Changed

- `MODULE.bazel`
- `BUILD.bazel`
- `crates/agenthub-acp-core/BUILD.bazel`
- `crates/agenthub-acp/BUILD.bazel`
- `crates/agenthub-team-actor/BUILD.bazel`
- `agenthub-codex-acp/BUILD.bazel`
- `Cargo.bazel.toml`
- `src/internal/mod.rs`
- `src/internal/proto/agenthub.internal.v1.rs`
- `Cargo.toml`
- `Cargo.lock`
- `.github/workflows/bazel.yml`
- `Makefile`
- `README.md`
- removed: `build.rs`
- removed: `bazel/ci/*`

## Validation

Expected commands:

```bash
bazel build //...
bazel test //...
```

Note: in restricted local sandbox environments, repository rule resolution may hang/fail due network/proxy/toolchain constraints. CI or a normal developer host should be used as the source of truth for full Bazel validation.

## Follow-ups

- Add native Bazel JS/web build+test targets (replace remaining npm-driven workflow gap).
