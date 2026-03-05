# Team Chat-First Golden Path E2E Case

## Background

The Team roadmap keeps an open verification item for the chat-first golden path:

- `task -> leader negotiation -> run compile -> worker execution -> final synthesis`

Existing Playwright coverage already validated Team Forge, mailbox IM flow, and compile-preview run-ops behavior independently, but there was no single scenario that chained compile-to-run with downstream worker/final-deliverable evidence in one test.

## Scope

- Added a new Playwright scenario in `web/tests/e2e/team_page.e2e.ts`:
  - `team chat-first path compiles preview, creates run, and captures worker plus final synthesis evidence`
- Expanded Team E2E API mocks:
  - Added `/api/agents/:id/.well-known/agent-card` mock response so Member Console discovery-card rendering remains deterministic under e2e mocks.
  - Added per-test run lifecycle mocks for:
    - compile preview endpoint
    - run creation/list/get
    - run snapshot/events
    - mailbox inbox/send/ack
- The new case validates:
  - compile preview result rendering;
  - `Create Run from Preview` request payload correctness;
  - mailbox conversation including worker status and leader follow-up message;
  - Events tab visibility of final synthesis payload;
  - Member Console discovery card capability tag rendering path.

## Key Decisions

1. Keep this case mock-driven.

- The test focuses on Team UI state transitions and API contract wiring, not backend runtime scheduling.
- Mock-driven flow keeps it deterministic and fast in CI.

2. Reuse existing Team E2E fixture surface.

- New route handlers are scoped to the case where needed.
- Shared fixture gains discovery-card route coverage to keep all Team E2E cases aligned with current frontend behavior.

3. Verify end-to-end semantics via user-visible signals.

- Assertions target rendered content and request payloads, not internal component state.

## Validation

Executed locally in this environment:

- `npm --prefix web run lint`
- `npm --prefix web run build`

Playwright execution is blocked in the current sandbox environment (Chromium launch permission / local webserver bind constraints), so full E2E runtime verification remains pending in CI or a permissive local runtime:

- attempted:
  - `npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts -g "team chat-first path compiles preview, creates run, and captures worker plus final synthesis evidence"`
  - `PLAYWRIGHT_NO_WEBSERVER=1 PLAYWRIGHT_PORT=5173 npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts -g "team chat-first path compiles preview, creates run, and captures worker plus final synthesis evidence"`
