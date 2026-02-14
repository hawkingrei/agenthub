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
- Allow opting out of webServer startup via `PLAYWRIGHT_NO_WEBSERVER=1` for
  component-level layout tests that use inline HTML/CSS only.

## Validation

```bash
cd web && npm run e2e
cd web && PLAYWRIGHT_NO_WEBSERVER=1 npx playwright test tests/e2e/input_dock_layout.e2e.ts
```
