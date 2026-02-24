# Unified Rust Version Pinning

## Summary

Pin Rust to a single version (`1.93.1`) across Bazel, rustup toolchain files,
and CI setup actions to avoid drift between local builds and CI.

## Background

The workspace previously mixed:

- Bazel toolchain pin (`MODULE.bazel`)
- `stable` rustup channels in CI
- sub-project rustup toolchain using `stable`

This can introduce non-deterministic behavior when dependencies raise MSRV.

## Scope

- `rust-toolchain.toml` (new, repo root)
- `agenthub-codex-acp/rust-toolchain.toml`
- `.github/workflows/bazel.yml`
- `.github/workflows/rust.yml`
- `docs/todo.md`

## Key Decisions

1. Use Rust `1.93.1` as the workspace baseline.
2. Keep CI setup actions pinned to the same version instead of `stable`.
3. Keep Bazel and rustup toolchains aligned to reduce "works locally but fails
   in CI" version skew.

## Validation

```bash
rustc --version
cargo --version
bazel build //...
bazel test //...
```

Expected: Rust tooling reports `1.93.1` for workspace commands, and CI does not
float to a newer `stable` unexpectedly.

## Follow-ups

- If dependency MSRV changes again, bump version once and update Bazel + rustup
  + CI together in a single PR.
