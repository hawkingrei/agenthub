## Summary

- disable cached test results in the `Bazel Coverage` workflow job
- keep normal Bazel build caching for compile and analysis work
- document the change and validation rationale

## Why

`bazel coverage` should execute tests fresh. Reusing cached test results weakens the coverage job as a verification signal and can hide coverage-specific runtime behavior.

## Testing

- `git diff --check`
