# PWA Support

Status: deprecated. PWA install/offline support was removed on 2026-02-08.
See `docs/journal/2026-02-08-pwa-removal.md`.

## Background

We want installable PWA support with an offline shell and a unified service worker that also handles push notifications.

## Scope

- Add `vite-plugin-pwa` with `injectManifest` strategy.
- Introduce a Workbox-based service worker with precache, navigation routing, and push handlers.
- Add PWA manifest and icons.
- Register the service worker at app startup.
- Disable auto-injected registration to avoid double registration.
- Use the `vite-plugin-pwa` v1 series to stay compatible with Vite 7.

## Key Decisions

- Use `injectManifest` to keep custom push handlers while enabling precache.
- Keep API and SSE requests out of the navigation cache path.
- Use a minimal icon set (192/512) for installability.
- Use explicit `registerSW()` in `main.tsx` as the single registration path.

## Validation

```bash
cd web && npm test
```

## Follow-ups

- Confirm install prompt behavior on Chrome/Safari.
- Validate offline shell behavior and cache updates.
