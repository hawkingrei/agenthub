# Frontend Performance Browser Baseline

## Summary

The local Vite frontend has browser-level baseline evidence for the unauthenticated workspace shell:
the `/workspace/teams` deep link redirects to login with preserved `next`, renders without
application console errors, and keeps a healthy lab LCP/CLS profile in dev mode. Authenticated
Playwright browser coverage now also exercises representative long Team channel and Team-member ACP
histories and verifies their initial DOM rendering stays windowed instead of mounting the full
history. The same authenticated long-history browser smoke now also runs against a local
production build served through `vite preview`.

## Background

The frontend performance hardening TODO already has focused hook and component regression coverage
for high-volume ACP and Team surfaces. It still needs browser/profiler evidence before making broad
page-level responsiveness claims. This checkpoint captures the first browser baseline plus a mocked
authenticated long-history browser smoke. It keeps the broader TODO open because it does not yet
cover deployed pages or profiler traces for live data.

## Scope

- Started the local Vite dev server on `http://127.0.0.1:5173/`.
- Opened `http://127.0.0.1:5173/workspace/teams`.
- Confirmed the app redirected to `/?next=%2Fworkspace%2Fteams` and exposed the expected login form
  in the accessibility tree.
- Collected console and performance trace summaries with Chrome DevTools MCP.
- Added an authenticated Playwright E2E fixture with:
  - 160 seeded Team channel messages;
  - 180 seeded Team-member ACP messages;
  - 90 seeded run-mailbox messages;
  - assertions that the latest message renders, the oldest source message is absent from mounted
    rows, and mounted row counts remain bounded.
- Extended the same fixture to simulate a user scroll-up on the Team channel timeline and verify
  the visible browser affordance switches to the jump-to-bottom state without losing the latest
  mounted tail row.
- Tightened the authenticated fixture so the Team channel and ACP open-path browser performance
  measures must be present and readable instead of silently defaulting missing measures to zero.
- Added a `PLAYWRIGHT_WEB_SERVER_COMMAND` override in the Playwright config so the same browser
  checks can run against either Vite dev mode or a local production preview server.
- Re-ran the authenticated long-history fixture against the local production bundle with
  `vite preview`.
- Confirmed the dev-server and production-preview runs use the same performance-measure existence
  guard for `team-channel-open`, `team-acp-open`, and `team-mailbox-open`.
- Did not claim deployed page performance from this baseline.

## Key Decisions

- Treat unauthenticated shell evidence as a health baseline only.
- Keep dev-mode network dependency findings separate from production bundle conclusions because Vite
  module loading creates a much larger request graph than a production build.
- Keep the frontend performance TODO open until deployed authenticated Team and ACP-heavy pages, or
  profiler traces with representative live data, confirm the same broad page-level behavior.

## Validation

```bash
npm run dev -- --host 127.0.0.1 --port 5173
PLAYWRIGHT_SYSTEM_CHROME=1 npx playwright test web/tests/e2e/team_page_performance.e2e.ts --project=system-chrome
npm run build
PLAYWRIGHT_PORT=4173 PLAYWRIGHT_WEB_SERVER_COMMAND="npm run preview -- --host 127.0.0.1 --port 4173 --strictPort" PLAYWRIGHT_SYSTEM_CHROME=1 npx playwright test tests/e2e/team_page_performance.e2e.ts --project=system-chrome
```

Chrome DevTools MCP evidence:

- Snapshot: root area `AgentHub`, login form, `Username`, `Password`, and `Login` controls at
  `http://127.0.0.1:5173/?next=%2Fworkspace%2Fteams`.
- Console: only Vite connection messages and the React DevTools informational message; no
  application errors.
- Performance trace for the same URL:
  - LCP: 223 ms
  - CLS: 0.00
  - TTFB: 5 ms
  - Network dependency tree max critical path latency: 152 ms
  - Estimated network-dependency savings: none

Playwright evidence:

- The default bundled Playwright Chromium cache was missing locally, so the first
  `npx playwright test web/tests/e2e/team_page_performance.e2e.ts --project=chromium` did not launch
  a browser.
- The same test passed with the configured `system-chrome` project.
- The test seeds 160 Team channel rows and verifies browser-mounted channel rows stay below 40 while
  the latest row is visible, source row 1 is not mounted, and the browser-side
  `team-channel-open` performance measure exists.
- The Team channel fixture now also scrolls the channel viewport away from the pinned bottom state
  and verifies the `Jump to bottom` control becomes visible while the latest tail row remains
  mounted.
- The test seeds 180 ACP rows for a selected Team member and verifies browser-mounted ACP rows stay
  below 40 while the latest row is visible, source row 1 is not mounted, and the browser-side
  `team-acp-open` performance measure exists.
- The test seeds 90 run-mailbox rows for a selected Team member and verifies browser-mounted
  mailbox rows stay below 40 while the latest row is visible, source row 1 is not mounted, and the
  browser-side `team-mailbox-open` performance measure exists.
- No page-level JavaScript errors were observed by the test.
- `npm run build` completed successfully and produced the production bundle.
- The same authenticated long-history Playwright test passed with `PLAYWRIGHT_WEB_SERVER_COMMAND`
  set to `npm run preview -- --host 127.0.0.1 --port 4173 --strictPort`, proving the fixture against
  the built assets rather than Vite dev module serving. This preview run also required
  `team-channel-open`, `team-acp-open`, and `team-mailbox-open` to exist.
- The sandboxed system-Chrome launch hit `kill EPERM`; the same production-preview command passed
  when rerun outside the sandbox.

## Follow-Ups

- Capture deployed authenticated Team workspace traces with representative channel, task, mailbox,
  member data, and manual scroll-up/jump-to-bottom interaction.
- Capture deployed or profiler-backed ACP-heavy conversation traces with long tool-call histories.
- Use deployed evidence before making CDN cache conclusions.
