# Bazel Build and Test Integration

## Background

The repository currently runs compile/test checks through direct `cargo` and `npm` commands in CI. We need a Bazel entrypoint so build and test workflows can be triggered through a single toolchain driver.

## Scope

- Add Bazel workspace bootstrap files.
- Add Bazel build targets for Rust/Web compilation and Bazel test targets for Rust/Web tests.
- Add a dedicated GitHub Actions workflow to run Bazel-driven checks.
- Keep existing Rust/Web workflows intact for coverage upload continuity.

## Key Decisions

- Use a minimal Bazel bootstrap (`MODULE.bazel` + root `BUILD.bazel`) to avoid a full migration to `rules_rust` and JS rules in this change.
- Expose explicit build/test targets:
  - Build: `//:rust_build`, `//:web_build`, aggregate `//:ci_build`
  - Test: `//:rust_test`, `//:web_test`, aggregate `//:ci_tests`
- Use local shell-based Bazel rules (`bazel/ci/defs.bzl`) so Bazel 9 does not depend on removed native `sh_binary` symbols.
- Run web steps in temporary copies to avoid writing into Bazel runfiles/sandbox source trees.
- Keep CI coverage collection in existing workflows and use the new Bazel workflow as an additional compile/test gate.

## Validation

```bash
bazel build //:ci_build
bazel test //:ci_tests
```

In CI:

- `.github/workflows/bazel.yml` should pass on both `push` to `main` and `pull_request`.
