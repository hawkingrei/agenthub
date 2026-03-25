# Bazel AgentHub Lib Managed Skills Dependency

## Summary

- Fixed the root `//:agenthub_lib` Bazel target to depend on `//crates/agenthub-managed-skills:agenthub_managed_skills`.
- This restores parity with `Cargo.toml`, where the root `agenthub` crate already depends on `agenthub-managed-skills`.

## Why

- `src/doctor_cli.rs` imports `agenthub_managed_skills::install_managed_skills`.
- Cargo builds succeeded because the root crate declared the dependency.
- Bazel failed with `unresolved import agenthub_managed_skills` because `BUILD.bazel` did not include the local crate edge for `//:agenthub_lib`.

## Validation

- `git diff --check`
- Local `bazel build //:agenthub_lib` was attempted, but this environment is currently blocked earlier by an unrelated Bazel module/package resolution error for `@@rules_rust+//rust:defs.bzl`.

