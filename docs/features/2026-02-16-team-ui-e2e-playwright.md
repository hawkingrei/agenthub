# Team UI Playwright E2E

## Background

`/teams` recently gained a staged Team Forge creation flow (Mission Brief -> Leader Forge ->
Recruit Workers -> Launch Team). Existing E2E coverage only checked login shell and input dock
layout, leaving Team UI regression risk unguarded.

## Scope

- Add Playwright E2E test `web/tests/e2e/team_page.e2e.ts`.
- Cover authenticated `/teams` page rendering via localStorage auth bootstrap.
- Mock Team-related API responses in-browser:
  - `GET /api/agents`
  - `GET /api/teams`
  - `POST /api/teams`
  - `GET /api/teams/:id/runs`
  - `GET /api/auth/status`
- Validate staged Team Forge flow:
  - Open modal from sidebar.
  - Complete mission/leader/worker stages.
  - Select leader/worker model presets.
  - Confirm generated spec contains expected workflow steps.
  - Submit create request and ensure team list updates.
- Validate Team Forge guardrail behavior:
  - reproduce duplicate member assignment by switching leader to an existing worker id,
  - assert stage-2 duplicate warning is visible,
  - assert `Next Stage` stays disabled until `Resolve Duplicates` is applied.
- Assert posted create payload includes expected leader/worker model selections and default step
  keys (`leader_plan`, `leader_synthesize`).

## Key Decisions

- Keep test self-contained by mocking network at Playwright route layer rather than requiring a
  real backend fixture.
- Use role/placeholder/text selectors plus limited structural selectors scoped inside dialog to
  keep assertions resilient to non-semantic CSS changes.
- Focus this test on Team Forge creation flow; run/steps/mailbox/member-console deeper interaction
  remains tracked as separate follow-up verification.

## Validation

Run:

```bash
HTTP_PROXY= HTTPS_PROXY= ALL_PROXY= NO_PROXY=127.0.0.1,localhost npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts --project=chromium
```
