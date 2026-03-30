# Bazel Codecov Env Tag

## Summary

Add an explicit Codecov env tag to the Bazel coverage upload so Bazel-originated
coverage reports can be filtered independently from the existing `flags: bazel`
metadata.

## Changes

- keep the existing Bazel Codecov `flags: bazel` upload grouping
- add `AGENTHUB_CODECOV_UPLOAD_TAG=bazel` to the Bazel coverage upload step
- pass that env var through `env_vars` so Codecov records it as upload metadata

## Validation

- inspect `.github/workflows/bazel.yml` and confirm the Bazel Codecov upload step
  now sets `AGENTHUB_CODECOV_UPLOAD_TAG`
- confirm `env_vars: AGENTHUB_CODECOV_UPLOAD_TAG` is present beside
  `flags: bazel`

