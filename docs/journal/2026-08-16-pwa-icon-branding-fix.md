# Summary

`web/public/pwa-192.png` and `web/public/pwa-512.png` -- the PWA install icons declared in
`manifest.webmanifest` and referenced as `apple-touch-icon` in `index.html` -- were solid navy placeholder
squares with no logo. Anyone who installed AgentHub as a PWA (home screen, taskbar, dock, app switcher)
got a blank colored tile instead of a recognizable icon. `index.html` also had no `<link rel="icon">` at
all, so the browser tab favicon was unbranded too. This is a distinct defect from the open PWA todo item
(deployed-domain Cloudflare `502` verification); that item is about confirming the *server contract*
survives production, not about the icon assets being wrong to begin with.

# Background

The real AgentHub brand mark already existed in the same directory,
`web/public/slock-icon.png` (180x180, used in the admin page UI, `admin_page_sections.tsx`), but was never
wired into the install icons -- `pwa-192.png`/`pwa-512.png` were an unrelated blank placeholder that
happened to match the manifest's `theme_color` (`#0f172a`), which is presumably why the gap went
unnoticed: the "icon" silently blended into a solid-color install prompt background instead of visibly
failing.

# Scope

- Regenerated `web/public/pwa-192.png` and `web/public/pwa-512.png` from `web/public/slock-icon.png` via
  Lanczos-resampled upscale (180x180 source; the design is bold and high-contrast, so it holds up
  acceptably at 512px).
- Added `<link rel="icon" href="/pwa-192.png" type="image/png" />` to `web/index.html`; there was
  previously no favicon link at all.
- `manifest.webmanifest`'s icon entries and `apple-touch-icon` already pointed at the right filenames, so
  no manifest or HTML `src` changes were needed beyond the new favicon link -- only the image bytes
  behind those existing references changed.

# Key Decisions

- Did not declare `"purpose": "any maskable"` on the manifest icons. The design has reasonable padding
  around the glyph, but verifying it actually fits Android/Chrome's maskable safe-zone (content within
  the inner ~80% diameter circle) needs real design tooling to confirm, not a visual guess. Declaring
  maskable support without being sure risks Android cropping the icon awkwardly, which would be worse
  than not declaring it.
- Did not add manifest `screenshots` for richer desktop install UI -- that requires real screenshots of
  the running app, not something to fabricate.
- Left the 180x180 source resolution as a known, documented limitation (`web-static-assets-and-pwa.md`
  Open Risks) rather than fabricating a higher-resolution source that doesn't exist.

# Validation

- `cd web && npm exec vitest -- run src/pwa_public_assets.test.ts` (2 passed) -- asserts manifest icon
  `src`/`sizes`/`type` stay stable; unaffected by the image-byte change, confirming this was purely an
  asset-content fix, not a contract change.
- `cd web && npm exec vitest -- run` (1510 passed, 161 files) -- full web suite, no regressions.
- `cd web && npm run build` -- succeeds; `dist/index.html` carries the new favicon link and
  `dist/pwa-192.png`/`dist/pwa-512.png` match the new `public/` bytes (verified by hash).
- `cargo test --lib web_cache_control_uses_immutable_for_hashed_asset_requests_only` and
  `cargo test --lib build_app_router_serves_file_system_fallback` -- both pass; the server-side cache and
  fallback contract from `web-static-assets-and-pwa.md` is untouched by this change.

# Follow-Ups

- The still-open PWA todo item (deployed-domain Cloudflare `502` verification of cache-control headers)
  is unrelated and unresolved by this fix.
- If the brand mark is ever redesigned, regenerate `pwa-192.png`/`pwa-512.png` from a native
  higher-resolution export rather than re-upscaling a 180px source.
