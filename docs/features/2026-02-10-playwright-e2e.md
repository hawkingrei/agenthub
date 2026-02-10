# Playwright E2E

## Background

UI regressions around workspace output and agents layout warrant a basic end-to-end smoke test in CI.

## Scope

- Add Playwright config and a minimal app shell test.
- Wire Playwright to CI with a Vite dev server.
- Pin Playwright via `@playwright/test` in `web` devDependencies.

## Key Decisions

- Use only Chromium to keep CI runtime reasonable.
- Use a dev server instead of preview build to reduce setup steps.

## Validation

```bash
cd web && npm run e2e
```
