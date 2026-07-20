# First-Run Web Setup Surface

## Summary

The browser login view now has a distinct first-run setup state when the
instance has no root operator. The surface creates only the initial root account
and keeps runtime role plus provider credential configuration outside the
browser path.

## Background

`agenthub init` already covers pre-start instance configuration. Operators can
still start the service before running it, so the first browser visit needs to
make the root bootstrap state explicit instead of looking like an ordinary login
form.

## Scope

- Show first-run setup language only when `/api/auth/status` reports that the
  root account is missing.
- Show a neutral setup check while root status is still loading, then fall back
  to the normal login shell if the status endpoint cannot be loaded.
- Keep the existing root registration API and form fields.
- State that server role and provider credentials remain operator-managed.
- Keep normal login language once a root account exists.

## Key Decisions

- The web surface is a root-account bootstrap panel, not a browser-side config
  writer.
- The loading state is only for an in-flight status check. Status endpoint
  failure should not leave the login shell permanently blocked.
- Provider API base URLs and API keys remain outside this slice until there is a
  reviewed config contract.
- Runtime role and internal gRPC settings stay in the local config file.

## Validation

```bash
cd web && npm exec vitest -- run src/routes/login_view.test.tsx
cd web && npm exec vitest -- run src/use_app_admin.test.tsx
cd web && npm exec playwright -- test tests/e2e/app.e2e.ts
```

## Follow-Ups

- Decide the provider credential and API base URL config contract before adding
  those fields to any setup surface.
- Add a safe browser-side instance config write path before exposing runtime
  role or internal gRPC setup in the web UI.
