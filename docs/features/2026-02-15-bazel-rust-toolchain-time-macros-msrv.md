# Bazel Rust Toolchain Bump for `time-macros` MSRV

## Summary

Fix Bazel Rust compilation failure caused by `time-macros 0.2.26` requiring
Rust 1.88+ while Bazel toolchain was pinned to 1.85.0.

## Background

The native `rules_rust` migration pins the workspace Rust toolchain through
`MODULE.bazel`. A CI failure showed:

- `let` chains reported as unstable
- `proc_macro_span` reported as unstable
- `integer_sign_cast` reported as unstable

for `external/.../time-macros-0.2.26`.

`time-macros 0.2.26` declares `rust-version = "1.88.0"`, so pinning Bazel to
1.85.0 is below MSRV and causes deterministic compile errors.

## Scope

- `MODULE.bazel`
- `docs/todo.md`

## Key Decisions

1. Bump `rust.toolchain.versions` from `1.85.0` to `1.88.0` (minimum required
   by current dependency graph).
2. Keep dependency set unchanged; resolve the failure at toolchain layer first.

## Validation

```bash
bazel build //...
bazel test //...
```

Expected: `time-macros` no longer fails with Rust unstable-feature errors on CI.

## Follow-ups

- If dependency graph raises MSRV again, prefer pinning Bazel Rust toolchain to
  the highest workspace MSRV and documenting the reason in feature notes.
