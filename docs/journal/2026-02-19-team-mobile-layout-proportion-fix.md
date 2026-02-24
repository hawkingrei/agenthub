# Team Mobile Layout Proportion Fix

## Background

Team workbench layout stayed in desktop proportions on some mobile and small-tablet viewports.
In practice this caused a squeezed sidebar/main split and narrow controls that were hard to use.

## Scope

- `web/src/styles.css`
- `web/tests/e2e/team_page.e2e.ts`

## Key Decisions

1. Add a Team-specific responsive breakpoint at `max-width: 960px` to force a single-column layout for the Team page.
2. Keep the existing global app breakpoints unchanged and only tune Team-related containers.
3. Add a tighter Team spacing pass at `max-width: 720px` for cards/lists/messages.
4. Add a Playwright regression test to assert mobile viewport behavior:
   - Team layout uses one column.
   - Run filter select keeps usable width.
   - Member row grid collapses to the expected compact column count.

## Validation Evidence (2026-02-19)

- Command:
  - `cargo test --test web_assets`
  - `/bin/zsh -lc 'set -e; PLAYWRIGHT_PORT=4173 npm run dev -- --host 127.0.0.1 --port 4173 --strictPort >/tmp/agenthub_vite_4173.log 2>&1 & dev_pid=$!; sleep 2; PLAYWRIGHT_NO_WEBSERVER=1 PLAYWRIGHT_PORT=4173 npm run e2e -- --grep "single-column proportions" tests/e2e/team_page.e2e.ts; rc=$?; kill $dev_pid >/dev/null 2>&1 || true; wait $dev_pid >/dev/null 2>&1 || true; exit $rc'`
- Result:
  - `web_assets` passed.
  - Playwright passed `team page keeps single-column proportions on mobile viewport`.

## Notes

- This keeps desktop Team proportions untouched.
- Real-device visual checks on iOS/Android are still recommended when available.
