# Bazel Root Rust Library Layout and RustEmbed Compile Data

## Summary

Refactor root Bazel Rust targets to use `rust_library` + `rust_binary` +
`rust_test(crate=...)`, and make RustEmbed compile inputs resilient when
`web/dist` is absent in CI checkout.

## Background

The root test target previously compiled `src/main.rs` directly as a `rust_test`.
This diverged from the library-first pattern used by projects like TypeDB and
made unit-test structure less explicit.

At the same time, opt-mode Bazel tests compile with `debug_assertions = false`,
so `#[derive(RustEmbed)]` is active. CI checkouts do not track `web/dist`,
which can break empty-glob evaluation and embed-folder resolution.

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
4. Switch embed root to `web/` and resolve requests by preferring `dist/*`.
5. Declare Bazel compile inputs as:
   - mandatory `web/index.html`
   - optional `web/dist/**` (`allow_empty = True`)
   so analysis and compilation both succeed when `web/dist` is missing.

## Validation

```bash
bazel build //:agenthub
bazel test --test_output=errors //:agenthub_unit_tests
```

Expected:

- Root Rust targets compile in Bazel without empty-glob or missing embed-folder errors.
- Root unit tests run through `rust_test(crate=...)` instead of `main.rs` crate-root tests.

## Follow-ups

- Keep the existing Cargo/Bazel proto toolchain unification follow-up before
  re-enabling Rust CI proto verification.
