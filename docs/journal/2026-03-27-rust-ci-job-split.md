# Rust CI Job Split

## Summary

- Split `.github/workflows/rust.yml` from one large `Rust (Cargo)` job into four narrower jobs:
  - `Rust (Cargo)` for workspace `cargo check`
  - `Rust (Proto Check)` for `make proto-check`
  - `Rust (gRPC Integration)` for the distributed TLS and in-process pipeline tests
  - `Rust (Coverage)` for `cargo llvm-cov` plus Codecov upload
- Kept the existing `Rust (Cargo)` check name so branch protection does not need an immediate rename.
- Disabled `rust-cache` target caching on `Rust (Proto Check)` and `Rust (Coverage)` because those jobs either duplicate compile work for validation only or build large coverage-instrumented artifacts that are poor cache candidates.

## Why

- The previous workflow packed codegen verification, workspace check, focused gRPC integration tests, and workspace coverage into one runner lifecycle.
- That made the single `rust-cache` payload large and slow to restore and save.
- It also hid which validation stage actually failed because every Rust concern surfaced through one job.

## Expected Outcome

- Smaller per-job cache footprints, especially for protobuf verification and coverage.
- Faster incremental feedback because `cargo check`, proto validation, and integration failures report independently.
- Less runner disk pressure in the main Rust workflow because coverage artifacts no longer share the same job cache lifecycle as regular checks.

## Validation

- Confirm the new checks appear on both `push` and `pull_request`:
  - `Rust (Cargo)`
  - `Rust (Proto Check)`
  - `Rust (gRPC Integration)`
  - `Rust (Coverage)`
- Record workflow run IDs here once both events pass.
