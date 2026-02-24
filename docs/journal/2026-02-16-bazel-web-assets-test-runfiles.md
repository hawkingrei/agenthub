# Bazel Web Assets Test Runfiles Compatibility

## Summary

Fix `//:web_assets_test` failure in Bazel CI by making the test resolve
`styles.css` from Bazel runfiles when repository-root paths are unavailable in
sandbox execution.

## Background

`tests/web_assets.rs` previously loaded:

- `web/src/styles.css` via `env!("CARGO_MANIFEST_DIR")`.

In Bazel test sandbox this path is not guaranteed to exist, causing:

- `styles.css should be readable: No such file or directory`.

## Scope

- `tests/web_assets.rs`
- `BUILD.bazel`
- `docs/todo.md`

## Key Decisions

1. Keep repository-root path as first choice for local/non-Bazel execution.
2. Add Bazel runfiles fallback path resolution using `TEST_SRCDIR` /
   `TEST_WORKSPACE`.
3. Declare `web/src/styles.css` in `web_assets_test` target `data` so runfiles
   always include the CSS file in CI.

## Validation

```bash
gh pr checks 22
```

Expected: `Bazel Build and Test` no longer fails at `//:web_assets_test` with
missing `styles.css`.

## Follow-ups

- If more web asset assertions are added, prefer centralizing runfiles path
  resolution helper logic for test reuse.
