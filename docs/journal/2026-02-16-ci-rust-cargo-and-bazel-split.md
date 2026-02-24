# CI Rust Cargo and Bazel Pipeline Split

## Summary

Keep Bazel workflow focused on native Bazel build/test and switch Rust workflow
back to Cargo-based compile/test coverage.

## Background

The repository keeps both `bazel` and `rust` workflows. Running Bazel commands in
both workflows duplicated responsibility and made Rust CI semantics unclear.

At the same time, Rust coverage uploads should be clearly distinguishable from
other pipelines in Codecov.

## Scope

- `.github/workflows/rust.yml`
- `src/internal/proto/agenthub.internal.v1.rs`
- `docs/todo.md`

## Key Decisions

1. Keep `.github/workflows/bazel.yml` as the only Bazel build/test workflow.
2. Change `.github/workflows/rust.yml` to Cargo flow:
   - `cargo check --workspace --locked`
   - `cargo llvm-cov --workspace --locked --lcov`
3. Use a dedicated Codecov flag for Cargo Rust coverage: `rust-cargo`.
4. Refresh tracked internal protobuf generated code to match current toolchain,
   so Cargo workspace compilation is consistent with current dependency versions.

## Validation

```bash
cargo check --workspace --locked
```

Expected:

- Workspace compiles under Cargo in CI-equivalent mode.
- Rust workflow no longer requires Bazel setup.
- Codecov upload for Rust uses `rust-cargo` flag.

## Follow-ups

- Evaluate caching strategy for `cargo-llvm-cov` installation in CI to reduce runtime.
