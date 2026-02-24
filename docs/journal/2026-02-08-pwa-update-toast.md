---
title: PWA Update Toast
date: 2026-02-08
status: implemented
---

Deprecated: PWA support was removed on 2026-02-08. This update flow is no
longer used. See `docs/journal/2026-02-08-pwa-removal.md`.

## Summary

Show a transient update bubble when a new service worker is available, then
auto-refresh the app to apply the update.

## Background

The PWA auto-update flow downloads a new service worker in the background but
does not surface a UI hint. Users had no indication that a newer build was
ready, and they often stayed on a stale version.

## Decision

- Register the service worker from the app root so the UI can react to
  `onNeedRefresh` and updates discovered by the browser.
- Poll `registration.update()` periodically to surface updates quickly.
- Display a small toast bubble when a new version is detected.
- Automatically refresh after a short delay to activate the update.

## Scope

- `web/src/app.tsx`
- `web/src/main.tsx`
- `web/src/styles.css`

## Validation

- Manual: simulate a new SW (rebuild/deploy), verify the toast appears and the
  page refreshes within a few seconds.
- Manual: wait for the periodic update check to trigger a refresh when a new
  service worker is available.
