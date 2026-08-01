# PWA Cache-Control Router Guard

## Summary

The filesystem-backed app router test now covers the full local PWA cache-control contract: SPA shell fallbacks stay `no-cache`, hashed assets stay immutable, `manifest.webmanifest` stays `no-cache`, `sw.js` stays `no-cache`, and missing hashed assets return `404` instead of falling back to the HTML shell.

## Background

AgentHub intentionally supports PWA installability without offline shell caching. The existing local tests covered shell fallback, hashed asset cache headers, and the manifest header. They did not prove that the app router serves `sw.js` with `no-cache`, and they did not prove that missing hashed asset URLs avoid shell fallback.

## Scope

- Extended `build_app_router_serves_file_system_fallback` in `src/app.rs`.
- Added a service-worker fixture to the temporary web directory.
- Added assertions for `/sw.js` `Cache-Control: no-cache`.
- Added assertions that a missing hashed asset returns `404` with `Cache-Control: no-cache`.
- Added a web public asset guard for installable manifest metadata and service worker scope.
- Added a canonical static asset/PWA feature spec.

## Key Decisions

- Keep this as app-router contract coverage rather than a production deployment claim.
- Keep missing asset requests outside shell fallback so an old asset URL cannot accidentally receive fresh HTML with the wrong cache semantics.
- Leave deployed-domain verification in `docs/todo.md` because CDN or reverse-proxy headers can still differ from the Rust router.

## Validation

Targeted check for this slice:

```bash
cargo test build_app_router_serves_file_system_fallback --lib
cd web && npm exec vitest -- run src/pwa_public_assets.test.ts
```

Production-domain retry on 2026-07-19 at 07:37 UTC:

```bash
curl -sS -D - -o /tmp/agenthub-pwa-manifest.out https://agenthub.hawkingrei.com/manifest.webmanifest
curl -sS -D - -o /tmp/agenthub-pwa-sw.out https://agenthub.hawkingrei.com/sw.js
curl -sS -D - -o /tmp/agenthub-pwa-root.out https://agenthub.hawkingrei.com/workspace/teams
curl -sS -D - -o /tmp/agenthub-pwa-missing-asset.out https://agenthub.hawkingrei.com/assets/__agenthub_missing_probe__.js
```

All four production probes returned Cloudflare `HTTP/2 502` with a 16-byte body. This only proves
the deployed entrypoint was unavailable during the retry; it does not prove the PWA cache-control
contract passed or failed.

Production-domain retry on 2026-07-19 at 09:47 UTC:

```bash
curl -L -sS -D - -o /tmp/agenthub-pwa-workspace-teams.html https://agenthub.hawkingrei.com/workspace/teams
curl -L -sS -D - -o /tmp/agenthub-pwa-sw.js https://agenthub.hawkingrei.com/sw.js
curl -L -sS -D - -o /tmp/agenthub-pwa-manifest.webmanifest https://agenthub.hawkingrei.com/manifest.webmanifest
curl -L -sS -D - -o /tmp/agenthub-pwa-missing-asset.txt https://agenthub.hawkingrei.com/assets/agenthub-missing-pwa-probe-20260719.js
wc -c /tmp/agenthub-pwa-workspace-teams.html /tmp/agenthub-pwa-sw.js /tmp/agenthub-pwa-manifest.webmanifest /tmp/agenthub-pwa-missing-asset.txt
```

All four production probes again returned Cloudflare `HTTP/2 502` with a 16-byte body. The
`Cache-Control` response was Cloudflare's private/no-store error response, not AgentHub's deployed
router contract, so the production PWA TODO remains blocked on a healthy entrypoint.

Production-domain retry on 2026-07-19 at 11:15 UTC:

```bash
curl -I -L https://agenthub.hawkingrei.com/workspace/teams
curl -I -L https://agenthub.hawkingrei.com/sw.js
curl -I -L https://agenthub.hawkingrei.com/manifest.webmanifest
curl -I -L https://agenthub.hawkingrei.com/assets/__missing-agenthub-pwa-probe__.js
```

All four production probes still returned Cloudflare `HTTP/2 502` with a 16-byte body. The response
headers again came from Cloudflare's error path, so this remains a production-entrypoint
availability blocker rather than PWA cache-control pass/fail evidence.

Production-domain retry on 2026-07-19 at 12:43 UTC:

```bash
curl -I --max-time 20 https://agenthub.hawkingrei.com/workspace/teams
curl -I --max-time 20 https://agenthub.hawkingrei.com/sw.js
curl -I --max-time 20 https://agenthub.hawkingrei.com/manifest.webmanifest
curl -I --max-time 20 https://agenthub.hawkingrei.com/assets/__missing-pwa-probe__.js
```

All four production probes still returned Cloudflare `HTTP/2 502` with a 16-byte body. This keeps
the deployed PWA TODO open: the observed headers are Cloudflare error-response headers, not evidence
that the deployed AgentHub router or CDN cache-control contract passed or failed.

Production-domain retry on 2026-07-19 at 13:04 UTC:

```bash
curl -I --max-time 20 https://agenthub.hawkingrei.com/workspace/teams
curl -I --max-time 20 https://agenthub.hawkingrei.com/sw.js
curl -I --max-time 20 https://agenthub.hawkingrei.com/manifest.webmanifest
curl -I --max-time 20 https://agenthub.hawkingrei.com/assets/__agenthub_missing_asset__.js
```

All four production probes still returned Cloudflare `HTTP/2 502` with a 16-byte body. The deployed
entrypoint is still unavailable, so production-domain PWA installability remains unverified.

Production-domain retry on 2026-07-19 at 13:23 UTC:

```bash
curl -I --max-time 20 https://agenthub.hawkingrei.com/workspace/teams
curl -I --max-time 20 https://agenthub.hawkingrei.com/sw.js
curl -I --max-time 20 https://agenthub.hawkingrei.com/manifest.webmanifest
curl -I --max-time 20 https://agenthub.hawkingrei.com/assets/__agenthub_missing_asset_probe_20260719_1323__.js
```

All four production probes still returned Cloudflare `HTTP/2 502` with a 16-byte body. The response
headers are still Cloudflare error-response headers, so they cannot prove the deployed router,
service-worker, manifest, missing-asset fallback, or CDN cache-control contract.

Production-domain retry on 2026-07-19 at 13:44 UTC:

```bash
/usr/bin/curl -sS -I --max-time 15 https://agenthub.hawkingrei.com/workspace/teams
/usr/bin/curl -sS -I --max-time 15 https://agenthub.hawkingrei.com/sw.js
/usr/bin/curl -sS -I --max-time 15 https://agenthub.hawkingrei.com/manifest.webmanifest
/usr/bin/curl -sS -I --max-time 15 https://agenthub.hawkingrei.com/assets/__agenthub_missing_probe__.js
```

All four production probes still returned Cloudflare `HTTP/2 502` with a 16-byte body and
Cloudflare error-response headers. Production PWA installability and cache-control behavior remain
unverified until the deployed entrypoint is healthy.

Production-domain retry on 2026-07-19 at 14:12 UTC:

```bash
/usr/bin/curl -sS -I --max-time 15 https://agenthub.hawkingrei.com/workspace/teams
/usr/bin/curl -sS -I --max-time 15 https://agenthub.hawkingrei.com/sw.js
/usr/bin/curl -sS -I --max-time 15 https://agenthub.hawkingrei.com/manifest.webmanifest
/usr/bin/curl -sS -I --max-time 15 https://agenthub.hawkingrei.com/assets/__agenthub_missing_probe_20260719_1410__.js
```

All four production probes still returned Cloudflare `HTTP/2 502` with a 16-byte body and
Cloudflare error-response headers. This remains an entrypoint availability issue, not evidence that
the deployed AgentHub router or CDN cache-control contract passed or failed.

Production-domain retry on 2026-07-20 at 20:08 UTC:

```bash
curl -L -I --max-time 20 https://agenthub.hawkingrei.com/workspace/teams
curl -L -I --max-time 20 https://agenthub.hawkingrei.com/sw.js
curl -L -I --max-time 20 https://agenthub.hawkingrei.com/manifest.webmanifest
curl -L -I --max-time 20 https://agenthub.hawkingrei.com/assets/__missing-agenthub-probe__.js
```

All four production probes still returned Cloudflare `HTTP/2 502` with a 16-byte body and
Cloudflare private/no-store error-response headers. This keeps the production-domain PWA
installability and cache-control TODO open until the deployed entrypoint is healthy enough to test
the AgentHub router and CDN headers.

## Follow-Ups

- Retry production response-header validation after the deployed entrypoint is healthy before
  closing the PWA installability TODO.
