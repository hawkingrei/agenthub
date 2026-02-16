# Connection Unavailable Banner Filter

## Summary

Hide the connection-recovery message from the global error banner and keep
connection state feedback in the header connection badge only.

## Background

The app already exposes network/SSE state via the connection badge (`Online ·
SSE ...` / `Offline · SSE ...`). Showing
`Connection unavailable (gateway response). Reconnecting...` again in
`ErrorBanner` duplicates status information and creates unnecessary visual
noise during reconnect loops.

## Scope

- `web/src/app.tsx`
- `web/src/connection_status.ts`
- `web/src/connection_status.test.ts`
- `docs/todo.md`

## Key Decisions

1. Keep sanitization behavior unchanged (`sanitizeErrorBannerMessage`) so error
   normalization remains centralized and predictable.
2. Add `shouldHideErrorBannerMessage` in `connection_status.ts` and treat
   `UPSTREAM_HTML_MESSAGE` as connection-status-only text.
3. In `App`, filter banner output after sanitization:
   - if message should be hidden, return `null` for `normalizedError`;
   - otherwise keep existing banner behavior.
4. Do not suppress other business/validation errors.

## Validation

Recommended command:

```bash
npm --prefix web run test -- src/connection_status.test.ts
```

Manual checks:

1. Simulate gateway/SSE interruption.
2. Confirm header badge shows reconnecting state.
3. Confirm `ErrorBanner` does not show
   `Connection unavailable (gateway response). Reconnecting...`.
4. Trigger a normal business error (for example invalid workdir) and confirm
   it still appears in `ErrorBanner`.
