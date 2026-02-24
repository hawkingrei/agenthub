# Cargo and Bazel Manifest Convergence

## Summary

Converge Bazel `crate_universe` dependency source to the workspace `Cargo.toml`
and remove the duplicated `Cargo.bazel.toml` manifest.

## Background

The repository previously maintained two independent Cargo manifests:

- `Cargo.toml` for Cargo workflows
- `Cargo.bazel.toml` for Bazel `crate_universe`

This split caused drift in dependency versions (notably `prost`/`tonic`),
leading to inconsistent behavior between Cargo and Bazel pipelines.

## Scope

- `MODULE.bazel`
- `Cargo.bazel.toml` (removed)
- `docs/todo.md`

## Key Decisions

1. Use `Cargo.toml` as the single dependency source of truth for both Cargo and Bazel.
2. Keep `Cargo.lock` as the lockfile input for `crate_universe`.
3. Remove `Cargo.bazel.toml` to eliminate dual-manifest drift risk.

## Validation

```bash
cargo check --workspace --locked
```

Expected:

- Cargo workspace compiles with locked dependencies.
- Bazel dependency resolution is derived from the same manifest as Cargo.

## Follow-ups

- Verify Bazel CI (`bazel build //...` and `bazel test //...`) remains stable
  after crate graph regeneration under unified manifest input.
