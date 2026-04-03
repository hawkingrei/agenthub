## Summary

This PR restores installable PWA metadata without reintroducing stale shell caching, and tightens Team conversation rendering around structured non-chat payloads.

### What changed

- restore installable PWA metadata and service worker registration without bringing back offline precache behavior
- serve `index.html`, SPA fallback routes, `sw.js`, and `manifest.webmanifest` with `Cache-Control: no-cache`
- serve hashed `/assets/*` bundles with `Cache-Control: public, max-age=31536000, immutable`
- collapse Team permission review cards once they are no longer pending, including payload-only timeout cases before polling catches up
- render Team `task_note` payloads as visible markdown text instead of raw JSON envelopes in Team conversation/mailbox views

## Validation

### Rust

- `cargo test build_app_router_serves_file_system_fallback --lib`
- `cargo test web_cache_control_uses_immutable_for_hashed_assets_only --lib`

### Web

- `cd web && npm run build`
- `cd web && npm run test -- src/pages/team/mailbox_helpers.test.ts src/pages/team_panels.test.tsx`

### Browser

- Chrome DevTools baseline on `https://agenthub.hawkingrei.com/teams/...` confirmed:
  - timed-out permission review cards still rendered their full action body
  - `task_note` payloads still rendered as raw JSON
- local DevTools regression on the rebuilt web shell confirmed:
  - `manifest.webmanifest` is linked from the document
  - `serviceWorker` registration succeeds at app startup
  - no new console errors were introduced by this change

## Docs

- added `docs/journal/2026-04-03-pwa-install-and-team-permission-card-collapse.md`
- added a follow-up deploy verification item to `docs/todo.md`
