# Retained Activation Transcripts

## Summary

Terminal loop activation records now open their recorded conversation in a read-only drawer.
The transcript remains available after the process exits and after a page reload.

## Background

Member history exposed durable outcomes and lifecycle details, while cold process diagnostics chose
a live execution owner. A completed native conversation could remain in the authorized event store
without a direct UI entry. Reusing the live diagnostics session override would also expose its input
flow to a historical selection.

## Scope

- Add `View transcript` for finished, interrupted and canceled records with a local session.
- Reuse the scoped event API, ACP parsing and conversation rendering in a separate drawer.
- Add abort support to the frontend event request, bounded paging and explicit refresh.
- Extend the actual native recovery browser case to read the completed conversation and reopen it
  after refreshing the member history page.

## Key Decisions

- Activation history supplies the exact recorded session; a newer live owner does not replace it.
- The transcript carries no input, approval or recovery callback and never starts a runtime.
- The existing backend event permissions remain authoritative. User/authentication, Team, member,
  activation and session changes invalidate pending reads; response identities are checked as well.
- A partial leading assistant reply remains hidden. At most one additional initial page is fetched;
  further reads are explicit. Zero-valued older cursors remain valid, and refresh starts a new chain.
- The drawer keeps the member overview route and pauses its background history polling while open.

## Validation

Focused transcript, history, member-panel and API selections pass 38 tests. They cover read-only
rendering, terminal-only navigation, retained session identity, zero cursors, refresh, bounded partial
chunk recovery, complete tails, denied reads and late responses after authentication/member changes.
TypeScript, ESLint and the production web build pass.

The actual native process/browser recovery case passes against real API/SSE handlers and a local
deterministic model. It completes recovery, finds the retained session from stored events, opens the
matching activation transcript, checks the assistant reply and absence of input/recovery controls,
then reloads and opens the same transcript again without sending input or another resolution.
The baseline screenshot shows only lifecycle-detail navigation. Playwright is used because Chrome
DevTools MCP is unavailable. Stable desktop and 390-pixel viewport screenshots were inspected after
the drawer transition completed; content and close controls fit both viewports, and no browser page
errors occurred. Evidence is retained under `target/loop-review-validation/retained-transcript-20261007b`.

```bash
cd web
npm exec vitest -- run src/pages/team/loop_activation_transcript.test.tsx src/pages/team/loop_activation_history.test.tsx src/pages/team/team_loop_member_panel.test.tsx src/api.test.ts
npm exec tsc -- --noEmit
npm run lint
npm run build
PLAYWRIGHT_NO_WEBSERVER=1 PLAYWRIGHT_MINIMAL_RUNTIME=1 npm exec playwright -- test tests/e2e/native_recovery_process.e2e.ts --project=chromium --workers=1
```

The browser command requires the existing native continuity backend fixture and matching
`LOOP_NATIVE_BROWSER_DIR`; see [native resume acceptance](2026-10-07-native-resume-acceptance.md).
Fixture manifests contain local authentication and remain private.

## Follow-Ups

- Record applicable exact-head CI with the published PR.
- Installed-provider and upstream publication gates remain tracked in [TODO](../todo.md).

Contract: [loop workspace UI](../features/agent-loop-workspace-ui.md).
