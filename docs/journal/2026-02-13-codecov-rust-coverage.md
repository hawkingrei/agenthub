# Codecov Rust Coverage Upload

## Background

Coverage trends were not visible in CI because the Rust pipeline only ran tests and did not publish a coverage report.

## Scope

- Generate an `lcov` report from Rust tests in the existing Rust workflow.
- Upload the generated report to Codecov so coverage can be tracked per run.
- Keep the workflow split model (`Rust`/`Web`/`Web E2E`) unchanged.

## Key Decisions

- Use `cargo-llvm-cov` to generate coverage in `lcov` format from the Rust workspace.
- Upload with `codecov/codecov-action@v5` using repository secret `CODECOV_TOKEN`.
- Keep `fail_ci_if_error: false` initially so CI remains usable while Codecov permissions are being verified.

## Validation

- In GitHub Actions `Rust` workflow:
  - `Generate Rust coverage report` should produce `lcov.info`.
  - `Upload Rust coverage to Codecov` should complete and publish the report with the `rust` flag.

## Follow-ups

- Web coverage upload is tracked separately in `docs/journal/2026-02-13-codecov-web-coverage.md`.
- Consider changing `fail_ci_if_error` to `true` after Codecov integration is stable.
