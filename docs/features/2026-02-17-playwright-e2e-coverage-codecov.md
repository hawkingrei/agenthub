# Playwright E2E Coverage Upload

## Summary

Add browser E2E JavaScript coverage collection for Playwright tests and upload it to Codecov as a dedicated `web-e2e` flag.

## Background

The current web coverage pipeline only reports Vitest unit coverage (`web` flag).  
Playwright E2E tests run in CI, but their exercised frontend paths are not reflected in coverage reports.

## Scope

- `web/tests/e2e/coverage.ts`
- `web/tests/e2e/coverage_merge.ts`
- `web/tests/e2e/coverage_merge.test.ts`
- `web/tests/e2e/*.e2e.ts`
- `web/playwright.config.ts`
- `web/package.json`
- `.github/workflows/e2e.yml`
- `docs/todo.md`

## Key Decisions

1. Add a shared Playwright test wrapper that conditionally enables browser JS coverage when `PLAYWRIGHT_E2E_COVERAGE=1`.
2. Collect only repository frontend source files (`/src/**`) to avoid dependency noise.
3. Convert Playwright V8 coverage entries into Istanbul coverage and emit `web/coverage/e2e/lcov.info`.
4. Keep E2E coverage in a separate Codecov flag (`web-e2e`) so it does not interfere with unit coverage gates.
5. Force single worker when E2E coverage is enabled to avoid cross-worker merge complexity.
6. Use a converter adapter that supports both modern `ast-v8-to-istanbul` API and legacy `load/applyCoverage` API shapes, and skip malformed/sourceless entries without failing the full E2E suite.

## Validation

```bash
npm --prefix web run e2e -- tests/e2e/app.e2e.ts --project=chromium
npm --prefix web run e2e:coverage -- tests/e2e/app.e2e.ts --project=chromium
```

Expected outcomes:

- E2E tests pass in both normal and coverage modes.
- Coverage mode generates `web/coverage/e2e/lcov.info`.
- CI E2E workflow uploads `web-e2e` coverage artifact to Codecov.
- Coverage merge does not fail with `converter.load is not a function` when dependency APIs change.

## Follow-ups

- Verify Codecov UI shows stable `web-e2e` flag trends on push/PR and evaluate whether patch gates should incorporate this flag.
- Verify CI reruns no longer show coverage merge crashes from sourceless scripts or converter API mismatch (`converter.load is not a function`).
