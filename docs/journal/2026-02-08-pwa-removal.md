---
title: Remove PWA Install Flow, Keep Push Service Worker
date: 2026-02-08
status: implemented
---

## Summary

Disable PWA install/offline caching while retaining a minimal service worker
for web push notifications.

## Background

PWA auto-update behavior caused confusion and stale UI issues. We decided to
remove the install/offline shell features and keep only the push notification
capability.

## Decision

- Remove `vite-plugin-pwa` and Workbox precache integration.
- Ship a minimal `public/sw.js` that handles push and notification clicks only.
- Keep push subscription registration in the app (`push.ts`) so notifications
  continue to work.

## Scope

- `web/vite.config.ts`
- `web/public/sw.js`
- `web/src/push.ts`
- `web/src/app.tsx`
- `web/src/styles.css`

## Validation

- Manual: trigger a push notification and confirm it arrives.
- Manual: ensure no PWA install prompt is presented.
