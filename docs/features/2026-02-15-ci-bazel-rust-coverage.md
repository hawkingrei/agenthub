# Rust CI Coverage via Bazel

## Background

The Rust workflow previously produced coverage via `cargo llvm-cov`, while the build/test direction moved toward Bazel-native execution. To align pipeline behavior with Bazel and keep Codecov `rust` flag reporting, the Rust workflow should collect coverage through Bazel.

## Scope

- Replace Rust workflow test execution with Bazel coverage execution.
- Generate a combined lcov report from Bazel.
- Upload Bazel-generated lcov report to Codecov under `rust` flag.

## Key Decisions

1. Use `bazel coverage --combined_report=lcov` on Rust Bazel test targets.
2. Keep explicit Rust target list (instead of `//...`) to avoid coupling Rust coverage to non-Rust jobs.
3. Materialize lcov as `bazel-rust.lcov` in workflow workspace before upload.
4. Keep `fail_ci_if_error: false` for Codecov upload parity with existing web workflow behavior.

## Workflow Changes

Updated `.github/workflows/rust.yml`:

- Build step: Rust Bazel binaries only.
- Coverage step: Bazel coverage over Rust test targets.
- Collection step: copy `$(bazel info output_path)/_coverage/_coverage_report.dat` to `bazel-rust.lcov`.
- Upload step: Codecov upload with `flags: rust`.

## Validation

Expected behavior in CI:

1. Rust workflow runs Bazel build and Bazel coverage successfully.
2. `bazel-rust.lcov` exists and is non-empty.
3. Codecov shows Rust upload under `rust` flag for PR/main runs.
