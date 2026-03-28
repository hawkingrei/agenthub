# Bazel CI Job Split And Coverage

## Summary

Split the Bazel workflow into smaller GitHub Actions jobs and add a dedicated Bazel coverage path
that uploads `bazel.lcov` to Codecov.

## Why

The previous Bazel workflow packed build and full test execution into a single runner job. That had
two downsides:

- the job cache accumulated normal build/test artifacts in one large lifecycle, which made the
  cache slower to restore and save;
- Bazel had no first-class coverage signal in CI, so Bazel-native test execution could regress
  without any dedicated `lcov` upload.

## Changes

- Replaced the single `Bazel Build and Test` runner job with four focused jobs:
  - `Bazel Build`
  - `Bazel Test (Root)`
  - `Bazel Test (Crates)`
  - `Bazel Coverage`
- Added an aggregate `Bazel Build and Test` job that depends on the four focused jobs so the
  existing top-level required check name can remain stable.
- Switched Bazel CI commands away from broad `//...` buckets where practical:
  - build job compiles the shipped binaries only
  - root tests and crate tests run as separate explicit target groups
- Gave each Bazel sub-job its own disk-cache key so coverage-instrumented artifacts do not bloat
  the regular build/test cache lifecycle.
- Added a Bazel coverage step:
  - `bazel coverage --combined_report=lcov ...` over the Rust Bazel test targets that emit
    coverage data
  - copy `$(bazel info output_path)/_coverage/_coverage_report.dat` to `bazel.lcov`
  - upload `bazel.lcov` to Codecov with flag `bazel`

## Validation

Local validation for this change should cover:

- `bazel test --test_output=errors //:web_assets_test`
- `bazel coverage --combined_report=lcov --test_output=errors //crates/agenthub-text:agenthub_text_tests`
- confirm `$(bazel info output_path)/_coverage/_coverage_report.dat` exists after the coverage run
- confirm GitHub Actions reports all four Bazel sub-jobs plus the aggregate `Bazel Build and Test`
  check on both `push` and `pull_request`

## Follow-up

- Record the first green `push` and `pull_request` run IDs for the split Bazel workflow before
  removing the verification item from `docs/todo.md`.
