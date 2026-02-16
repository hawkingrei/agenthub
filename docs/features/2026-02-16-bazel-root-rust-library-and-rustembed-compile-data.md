# Bazel Root Rust Library Layout and RustEmbed Compile Data

## Summary

Refactor root Bazel Rust targets to use `rust_library` + `rust_binary` +
`rust_test(crate=...)`, and wire `web/dist` into compile-time inputs so
`RustEmbed` can compile under Bazel opt/test sandbox builds.

## Background

The root test target previously compiled `src/main.rs` directly as a `rust_test`.
This diverged from the library-first pattern used by projects like TypeDB and
made unit-test structure less explicit.

At the same time, opt-mode Bazel tests compile with `debug_assertions = false`,
so `#[derive(RustEmbed)]` for `web/dist` is active. In Bazel sandbox builds,
that folder is not available unless declared as compile input.

## Scope

- `BUILD.bazel`
- `src/lib.rs`
- `src/main.rs`
- `docs/todo.md`

## Key Decisions

1. Introduce a root `rust_library` target (`agenthub_lib`) with
   `crate_root = "src/lib.rs"` and `crate_name = "agenthub"`.
2. Keep `agenthub` as a thin `rust_binary` that calls `agenthub::run()`.
3. Convert `agenthub_unit_tests` to `rust_test(crate = ":agenthub_lib")` so
   unit tests run against the library crate.
4. Declare `compile_data = glob(["web/dist/**"])` on the root library target,
   ensuring `RustEmbed` sees embedded assets in Bazel sandbox compilation.

## Validation

```bash
bazel build //:agenthub
bazel test --test_output=errors //:agenthub_unit_tests
```

Expected:

- Root Rust targets compile in Bazel without `RustEmbed` missing-folder errors.
- Root unit tests run through `rust_test(crate=...)` instead of `main.rs` crate-root tests.

## Follow-ups

- Keep the existing Cargo/Bazel proto toolchain unification follow-up before
  re-enabling Rust CI proto verification.
