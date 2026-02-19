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
  - Create leader/worker agents through Agent Forge entry and bind them into Team Forge flow.
  - Select leader/worker model presets.
  - Confirm generated spec contains expected workflow steps.
  - Submit create request and ensure team list updates.
- Validate Team Forge guardrail behavior:
  - reproduce duplicate member assignment inside Team Forge flow (same forged member bound to
    both leader and worker),
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

Executed (2026-02-18):

```bash
npm --prefix web run test -- src/pages/team_page.runs.test.ts
/bin/zsh -lc 'set -euo pipefail; npm --prefix web run dev -- --host 127.0.0.1 --port 5173 --strictPort >/tmp/agenthub-vite.log 2>&1 & VITE_PID=$!; trap "kill $VITE_PID 2>/dev/null || true" EXIT; for i in {1..30}; do curl -sf http://127.0.0.1:5173 >/dev/null && break; sleep 1; done; PLAYWRIGHT_NO_WEBSERVER=1 npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts'
npm --prefix web run build
```
