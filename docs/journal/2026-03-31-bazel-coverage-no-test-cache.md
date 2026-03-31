## Summary

Adjusted the `Bazel Coverage` GitHub Actions job to disable Bazel test result caching while keeping normal build/action cache behavior.

## Why

Coverage runs should execute tests fresh instead of reusing cached test results. Reusing cached test results can hide coverage-specific runtime behavior and make the coverage job less trustworthy as a verification signal.

## Change

- Updated [`.github/workflows/bazel.yml`](../../.github/workflows/bazel.yml) so `bazel coverage` runs with:
  - `--combined_report=lcov`
  - `--test_output=errors`
  - `--nocache_test_results`

This keeps Bazel build/action caches available for compilation and analysis work, while forcing test execution during the coverage job.

## Validation

- Reviewed the workflow diff to confirm only the coverage invocation changed.
- Ran `git diff --check`.
