# Web Static Assets And PWA

## Problem

AgentHub should remain installable as a browser app without reintroducing stale shell caching.
Historically, offline shell precaching made deploys and local rebuilds confusing because refreshes
could keep serving old HTML or old asset graphs.

## Scope

- Browser install metadata in the web shell.
- Static web asset routing from the Rust server.
- Cache-control behavior for shell routes, manifest, service worker, and hashed assets.
- Service worker scope for push and notification interactions.

## Non-Goals

- Offline-first application shell caching.
- Workbox or generated precache manifests.
- Background sync or offline mutation queues.
- Release-site verification for a specific deployed domain.

## Architecture

- `web/index.html` owns install metadata links and mobile app meta tags, including the browser-tab
  favicon (`<link rel="icon">`), not just install-time metadata.
- `web/public/manifest.webmanifest` owns web app manifest metadata and icons.
- `web/public/pwa-192.png` and `web/public/pwa-512.png` are generated from the real brand asset
  (`web/public/slock-icon.png`, also used in the admin page UI), not authored as standalone install
  icons; regenerate them from that source (or a higher-resolution successor) if the brand mark changes.
- `web/public/sw.js` is a minimal service worker for lifecycle, push, and notification-click
  behavior.
- `src/web.rs` owns request-path-based static asset fallback and cache-control classification.
- `src/app.rs` wires the filesystem or embedded static handler into the application router.

## Contracts

- The service worker must not intercept `fetch` or precache navigation responses.
- HTML shell routes and SPA fallback responses must use `Cache-Control: no-cache`.
- `/sw.js` must use `Cache-Control: no-cache`.
- `/manifest.webmanifest` must use `Cache-Control: no-cache`.
- Hashed `/assets/*` build artifacts may use
  `Cache-Control: public, max-age=31536000, immutable`.
- Missing `/assets/*` requests must not fall back to `index.html`; they should fail as missing
  assets so stale HTML is not served under an immutable asset URL.
- The manifest should preserve installable app metadata: `name`, `short_name`, `start_url`,
  `scope`, `display`, theme/background colors, and PNG icons.

## Validation Matrix

- `cargo test web_cache_control_uses_immutable_for_hashed_asset_requests_only --lib`
- `cargo test build_app_router_serves_file_system_fallback --lib`
- `cd web && npm exec vitest -- run src/pwa_public_assets.test.ts`
- `cd web && npm run build`
- Browser deployment checks should confirm:
  - shell routes return `Cache-Control: no-cache`;
  - `/sw.js` returns `Cache-Control: no-cache`;
  - `/manifest.webmanifest` returns `Cache-Control: no-cache`;
  - hashed `/assets/*` return immutable cache headers;
  - missing hashed assets do not return the HTML shell.

## Operational Notes

- Installability and offline support are separate. AgentHub supports installability without an
  offline navigation cache.
- Deploy validation should inspect response headers after the production artifact is actually
  published; local app-router tests prove the server contract but not CDN/proxy behavior.

## Open Risks

- A CDN or reverse proxy can override the Rust server's cache headers.
- Browser installability can regress if manifest icons or shell metadata drift.
- `pwa-192.png`/`pwa-512.png` are upscaled from a 180x180 source (`slock-icon.png`); the 512px icon is
  visually acceptable for this bold, high-contrast flat design but a native higher-resolution export
  would render more crisply if the brand asset is ever redesigned.
- A future service worker change could accidentally add fetch handling and reintroduce stale shell
  caching.

## Source Journals

- [docs/journal/2026-02-08-pwa-removal.md](../journal/2026-02-08-pwa-removal.md)
- [docs/journal/2026-04-03-pwa-install-and-team-permission-card-collapse.md](../journal/2026-04-03-pwa-install-and-team-permission-card-collapse.md)
- [docs/journal/2026-07-19-pwa-cache-control-router-guard.md](../journal/2026-07-19-pwa-cache-control-router-guard.md)
