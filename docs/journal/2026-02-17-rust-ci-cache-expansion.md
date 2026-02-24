# Rust CI Cache Expansion

## Summary

Add Cargo/Rust build cache to the Rust workflow and make `cargo-llvm-cov` setup cache-aware.

## Background

The Rust workflow currently reinstalls and recompiles dependencies more often than needed.  
`Clippy` workflow already uses `Swatinem/rust-cache`, but `Rust (Cargo)` workflow did not.

## Scope

- `.github/workflows/rust.yml`
- `docs/todo.md`

## Key Decisions

1. Reuse the existing repository-standard cache action (`Swatinem/rust-cache@v2`) in Rust workflow.
2. Keep `cargo llvm-cov` flow unchanged, but install `cargo-llvm-cov` only when not already available in PATH.
3. Keep explicit version pinning (`dtolnay/rust-toolchain@1.93.1`) and current coverage upload flag (`rust-cargo`) unchanged.

## Validation

```bash
gh run view --log --workflow rust.yml
```

Expected outcomes:

- Rust workflow shows cache restore/save events.
- `Setup Rust coverage tool` skips reinstall when `cargo-llvm-cov` is already cached.
- `cargo llvm-cov` and Codecov upload steps still pass.

## Follow-ups

- Observe 3-5 consecutive CI runs to confirm reduced compile/install time and stable cache hit rates.
