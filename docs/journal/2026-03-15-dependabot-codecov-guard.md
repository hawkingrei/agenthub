## Summary

Dependabot pull requests in this repository were failing `Rust (Cargo)`, `Web`, and `Web E2E` after the real build/test work completed.

## Root Cause

The failing step was the shared Codecov upload action in all three workflows.

- Dependabot-triggered `pull_request` runs do not receive the repository `CODECOV_TOKEN` through normal Actions secrets.
- The repository branch protection and Codecov configuration require a token for uploads on these branches.
- Each workflow set `fail_ci_if_error: true`, so the missing-token upload failure turned an otherwise successful job red.

## Change

Added a workflow guard so Codecov upload steps are skipped for Dependabot pull requests while leaving normal PR and `main` push uploads unchanged.

## Validation

- Inspect `Rust (Cargo)`, `Web`, and `Web E2E` logs for PR `#123`: each job completed build/test work and failed only at Codecov upload with `Token required because branch is protected`.
- Verify the new workflow condition is present in:
  - `.github/workflows/rust.yml`
  - `.github/workflows/web.yml`
  - `.github/workflows/e2e.yml`
