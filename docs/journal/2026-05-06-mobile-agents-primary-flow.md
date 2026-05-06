# Mobile Agents Primary Flow

## Summary

Added explicit mobile browser coverage for the Agents primary workbench flow so the small-screen
contract is no longer represented only by Team and Nodes checks. The same slice also keeps the
workspace header above the mobile agents backdrop so the compact pane toggle remains clickable after
opening the agents list.

## Background

`docs/features/frontend-design.md` requires dedicated narrow-screen coverage for Team, Agents, and
Nodes primary flows. Existing mobile e2e coverage already guarded Team shell proportions, Team
setup actions, and Nodes list/detail/connect-command surfaces. The remaining obvious gap was a
browser-level Agents path that proves the agent list and active workbench remain reachable on a
phone-sized viewport.

## Scope

- Covers `/workspace` on a `390x844` viewport.
- Uses the existing Team-page e2e fixture because it already provides authenticated root state,
  agents, nodes, and Team API mocks.
- Adds agent event and input route mocks specific to the Agents workbench path.
- Raises the authenticated workspace header above the mobile agents backdrop.

## Key Decisions

- Keep the test focused on primary mobile affordances instead of visual pixel assertions.
- Treat the header sidebar toggle as the compact pane switch between agent list and active
  workbench content.
- Verify a real input submit reaches `/api/agents/:id/input`, not just that the input dock renders.
- Keep the header at `z-40`, above the `z-20` mobile backdrop and `z-30` agents panel, so the
  header toggle is not visually available but pointer-blocked.

## Validation

Focused validation:

```bash
cd web && PLAYWRIGHT_NO_WEBSERVER=1 PLAYWRIGHT_MOBILE_ONLY=1 npm exec -- playwright test --project chromium
npm --prefix web run lint
npm --prefix web exec -- tsc -p web/tsconfig.json --noEmit
npm --prefix web run build
```

The local Playwright run used an already-started Vite dev server on `127.0.0.1:5173`; Chromium had
to run outside the filesystem sandbox on macOS because the sandbox blocked Chromium's Mach port
registration.

## Follow-Ups

- Continue broadening small-screen coverage for Team `Conversation <-> Kanban <-> thread` once the
  remaining channel/thread UX work lands.
- Keep Chrome DevTools MCP notes attached to PRs that change visible compact layout behavior, not
  only test coverage.
