# Bazel Build and Test Integration

## Background

The repository currently runs compile/test checks through direct `cargo` and `npm` commands in CI. We need a Bazel entrypoint so build and test workflows can be triggered through a single toolchain driver.

## Scope

- Add Bazel workspace bootstrap files.
- Add Bazel targets that orchestrate Rust and Web checks.
- Add a dedicated GitHub Actions workflow to run Bazel-driven checks.
- Keep existing Rust/Web workflows intact for coverage upload continuity.

## Key Decisions

- Use a minimal Bazel bootstrap (`MODULE.bazel` + root `BUILD.bazel`) to avoid a full migration to `rules_rust` and JS rules in this change.
- Expose three Bazel run targets:
  - `//:rust_checks` for `cargo build --workspace` and `cargo test --workspace`
  - `//:web_checks` for `npm ci`, `npm run lint`, `npm run test`, and `npm run build`
  - `//:ci_checks` to run Rust and Web checks in sequence
- Keep CI coverage collection in existing workflows and use the new Bazel workflow as an additional compile/test gate.

## Validation

```bash
bazel run //:rust_checks
bazel run //:web_checks
bazel run //:ci_checks
```

In CI:

- `.github/workflows/bazel.yml` should pass on both `push` to `main` and `pull_request`.
