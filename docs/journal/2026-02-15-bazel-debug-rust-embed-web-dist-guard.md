# Bazel Debug Build Guard for `rust-embed` Web Assets

## Summary

Prevent Bazel debug/test builds from failing when `web/dist` is absent by
compiling `RustEmbed`-based web embedding only for non-debug builds.

## Background

`src/web.rs` used:

- `#[derive(RustEmbed)]`
- `#[folder = "web/dist"]`

`rust-embed` validates the folder at compile time. In Bazel sandboxed debug
builds (`bazel test //...`), `web/dist` is not guaranteed to exist, causing:

- `folder '.../web/dist' does not exist`
- follow-up missing `EmbeddedWeb::get` errors

This is a build-input mismatch, not a runtime router bug.

## Scope

- `src/web.rs`
- `docs/todo.md`

## Key Decisions

1. Gate `RustEmbed` imports/derive and the real embedded handler with
   `#[cfg(not(debug_assertions))]`.
2. Provide a debug-only `embedded_handler` stub that returns `404` to keep API
   surface stable during debug/test compilation.
3. Preserve release behavior: release builds still compile embedded assets and
   require `web/dist` as designed.

## Validation

```bash
bazel test --test_output=errors //:agenthub_unit_tests
bazel test --test_output=errors //...
```

Expected: debug/test Bazel compile no longer requires `web/dist`, while release
embedding behavior remains unchanged.

## Follow-ups

- If we require `-c opt` Bazel builds without prebuilt frontend artifacts,
  connect release embedding to a Bazel-managed web build output instead of
  relying on workspace `web/dist`.
