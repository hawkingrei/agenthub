# Storage Quota Guard For Web Runtime

## Background

The web runtime persists output cache and small UX state in `localStorage`.  
On long-running sessions, some browsers can hit storage quota limits and throw `DOMException: The quota has been exceeded`.

When this happens during a `setItem` call in a render-adjacent path, the UI can become unstable.

## Scope

- Harden output cache persistence with quota-aware fallback writes.
- Guard auth/input-history local storage writes against storage exceptions.
- Keep runtime behavior stable when storage is unavailable or full.

## Key Decisions

1. Keep output cache best-effort:
   - Retry persistence with reduced payload shapes.
   - Drop ACP cache first in fallback attempts.
   - If quota remains exceeded, clear persisted output cache key.
2. Treat auth/input-history persistence as non-fatal:
   - Wrap read/write/remove operations in safe helpers.
   - Never throw from storage operations into app flow.
3. Keep no-op behavior for non-quota storage failures.

## Validation

- Added/updated web unit tests in `web/src/output_cache_storage.test.ts`:
  - retries after one quota failure;
  - does not throw when quota remains exceeded.
- Run:
  - `npm --prefix web test -- output_cache_storage`

## Follow-up

- Verify in a real browser with near-full storage that:
  - conversation rendering keeps working;
  - login/join/session actions do not white-screen;
  - output cache degrades gracefully without repeated uncaught exceptions.
