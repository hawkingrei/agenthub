# Codecov Web Coverage Upload

## Background

The Web CI pipeline ran tests but did not publish coverage artifacts, so frontend coverage trends were not visible in Codecov.

## Scope

- Add Vitest coverage dependency for the web workspace.
- Add a dedicated coverage test script that emits `lcov`.
- Upload the web `lcov` report to Codecov from the `Web` workflow.

## Key Decisions

- Use `@vitest/coverage-v8` to align with Vitest v4 default provider support.
- Keep existing `test` script unchanged and add `test:coverage` for CI coverage publishing.
- Upload with `codecov/codecov-action@v5` using `CODECOV_TOKEN` and `web` flag.
- Keep `fail_ci_if_error: false` during initial rollout to avoid blocking CI on transient Codecov issues.

## Validation

```bash
cd web
npm run test:coverage
```

- The command should generate `web/coverage/lcov.info`.
- In GitHub Actions `Web` workflow, `Upload coverage reports to Codecov` should publish the report with the `web` flag.
