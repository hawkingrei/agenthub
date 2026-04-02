---
title: Installable PWA Without Stale Caching And Team Permission Card Collapse
date: 2026-04-03
status: implemented
---

## Summary

Restore installable PWA support without reintroducing offline shell caching,
collapse Team permission review cards once they time out or are answered, and
render structured Team `task_note` payloads as markdown text instead of raw
JSON envelopes. At the same time, slim down the Team workbench shell so header
and wrapper chrome leave more room for the primary content panes.

## Background

AgentHub previously removed Workbox-style PWA/offline behavior because cached
HTML and shell assets caused stale UI confusion after deploys and local web
rebuilds. We still want installability, but the page refresh path must keep
picking up the newest `index.html` and hashed asset graph.

Separately:

- timed-out Team permission review cards continued to render their full action
  body in the shared conversation even after the review had already expired,
  which left the channel visually noisy and misleading;
- Team `task_note` payloads such as worker idle updates and task results were
  falling through the generic JSON fallback in `TeamTaskPanel`, so the shared
  channel showed the raw envelope instead of rendering `payload.text` through
  the normal markdown path.
- The Team workbench shell itself still consumed more space than it should:
  large radii, heavier shadows, and generous wrapper padding made the header
  and workspace chrome compete with the conversation/task content.

## Decisions

- Keep the service worker minimal:
  - register it at app startup so installability is not gated on push opt-in;
  - retain only lifecycle, push, and notification-click behavior;
  - do not intercept `fetch` or precache HTML/navigation responses.
- Add a web app manifest and standard install metadata to the root HTML shell.
- Apply cache headers by request path:
  - `/assets/*` uses `public, max-age=31536000, immutable`;
  - `index.html`, SPA fallback routes, `sw.js`, and `manifest.webmanifest` use
    `no-cache`.
- Collapse Team permission cards to a compact header-only state when the card is
  no longer pending, including the common `review_timeout` payload-only case
  before permission polling has refreshed the backend record.
- Treat Team `task_note` payloads as visible conversation text in the same
  parser layer as `chat_message`, so channel/mailbox views feed `payload.text`
  into the existing markdown renderer instead of pretty-printing JSON.
- Tighten the Team workbench shell chrome in `team_page.tsx` by reducing the
  outer section card radius/padding/shadow and slimming the header/workspace
  wrapper classes.

## Scope

- `src/app.rs`
- `src/web.rs`
- `web/index.html`
- `web/public/manifest.webmanifest`
- `web/src/main.tsx`
- `web/src/push.ts`
- `web/src/pages/team_task_panel.tsx`
- `web/src/pages/team_mailbox_panel.tsx`
- `web/src/pages/team/mailbox_helpers.ts`
- `web/src/pages/team/mailbox_helpers.test.ts`
- `web/src/pages/team_page.tsx`
- `web/src/pages/team_panels.test.tsx`
- `docs/todo.md`

## Validation

- Rust:
  - `cargo test build_app_router_serves_file_system_fallback --lib`
  - `cargo test web_cache_control_uses_immutable_for_hashed_assets_only --lib`
- Web:
  - `cd web && npm run build`
  - `cd web && npm run test -- src/pages/team/mailbox_helpers.test.ts src/pages/team_panels.test.tsx`
- Chrome DevTools MCP:
  - Baseline on `https://agenthub.hawkingrei.com/teams/...` showed timed-out
    permission cards still rendering full summaries and action buttons, and
    older `task_note` rows still rendering as raw JSON envelopes.
  - Post-edit local regression should confirm:
    - the generated shell exposes `manifest.webmanifest`;
    - no new console errors are introduced in the local preview shell;
    - timed-out/responded permission cards render compact status rows only;
    - `task_note` payloads render as markdown text via the existing thread-rich
      text path rather than raw JSON.

## Follow-up

- Verify deployed installability and refresh behavior on
  `agenthub.hawkingrei.com` after merge: the browser should show installable PWA
  metadata while a new `make build-web` / deploy should still take effect on
  refresh without stale shell assets.
