# 2026-03-16 Auth WebAuthn Request Guard

## Context

Chrome DevTools MCP inspection on `https://agenthub.hawkingrei.com/` showed a persistent
`A request is already pending.` error banner at the top of the authenticated workbench while the
active Codex agent had no pending ACP permission requests and no fresh ACP prompt errors in recent
agent events.

The login flow in `web/src/app.tsx` had no busy guard around `navigator.credentials.get()` or
`navigator.credentials.create()`. Repeated clicks could therefore start overlapping WebAuthn
requests, and the browser-level error message leaked into the global error banner.

## Changes

- added an auth request guard in `web/src/app.tsx` using a synchronous `authBusyRef` plus visible
  `authBusy` state so repeated login/register clicks are ignored until the current request settles;
- disabled login/register form inputs while an auth request is active;
- updated the auth action labels to show `Logging in...` / `Bootstrapping...` while the request is
  in flight;
- added a focused regression test in `web/src/app.runtime_effects.test.tsx` to verify a second
  login click does not trigger a second `loginStart` or WebAuthn credential request while the
  first one is still pending.

## Validation

- `cd web && npx vitest run src/app.runtime_effects.test.tsx`
- `cd web && npm run lint -- src/app.tsx src/app.runtime_effects.test.tsx`
- Chrome DevTools MCP baseline on the live domain confirmed the visible error banner and showed no
  active pending ACP permission items for the active Codex agent at inspection time.
