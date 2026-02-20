# CI Codecov Fail-Fast Upload Mode

## Background

Codecov upload comments previously showed intermittent "missing upload" observations.
The workflow-level setting `fail_ci_if_error: false` allowed jobs to pass even when
coverage uploads failed, which can hide upload regressions.

## Scope

- `.github/workflows/rust.yml`
- `.github/workflows/web.yml`
- `.github/workflows/e2e.yml`
- `docs/todo.md`

## Changes

1. Switched Codecov upload steps to strict mode by setting:

```yaml
fail_ci_if_error: true
```

2. Applied the same policy to all current coverage upload jobs:

- `rust-cargo` flag upload in `Rust` workflow
- `web` flag upload in `Web` workflow
- `web-e2e` flag upload in `Web E2E` workflow

## Decision

- Use a fail-fast policy for coverage upload reliability.
- If upload fails, fail the CI job immediately rather than producing a green check with missing coverage data.

## Validation Plan

- Verify `Rust`, `Web`, and `Web E2E` workflows succeed on both `push` and `pull_request` events.
- Confirm Codecov shows all three expected uploads/flags for the same commit:
  - `rust-cargo`
  - `web`
  - `web-e2e`
- Record workflow run IDs before marking the TODO item done.

