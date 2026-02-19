# Team UI State Reducer Phase 2 (Run + Step Controls)

## Summary

Extend Team page reducer migration to include run/step control form state:
- `runContextId`
- `runInput`
- step submit fields (`stepKey`, `stepMemberId`, `stepDependsOn`, `stepInput`)
- step action controls (`selectedStepId`, `stepAction`, `stepRemoteTaskId`, `stepOutput`, `stepFailText`, `stepInputReason`, `stepInputRequiredPayload`, `stepResumePayload`)

## Background

Phase 1 moved stable UI selectors (`tab`, `runLookupId`, `eventsAutoRefresh`) into reducer. Run and step controls were still spread across many `useState` atoms. This phase reduces control-state fragmentation without changing runtime behavior.

## Scope

- `web/src/pages/team_page.tsx`
- `docs/todo.md`

## Key Decisions

1. Introduce `TeamControlState` + `TeamControlAction` (`patch`) for run/step controls only.
2. Keep existing setter call sites by exposing callback wrappers (`setRunContextId`, `setStepKey`, etc.) over reducer dispatch.
3. Avoid mailbox/create-team migration in this phase to limit blast radius.

## Validation

Executed (2026-02-19):

```bash
npm --prefix web run lint
npm --prefix web run test -- src/pages/team_page.runs.test.ts
npm --prefix web run build
/bin/zsh -lc 'set -euo pipefail; npm --prefix web run dev -- --host 127.0.0.1 --port 5173 --strictPort >/tmp/agenthub-vite.log 2>&1 & VITE_PID=$!; trap "kill $VITE_PID 2>/dev/null || true" EXIT; for i in {1..30}; do curl -sf http://127.0.0.1:5173 >/dev/null && break; sleep 1; done; PLAYWRIGHT_NO_WEBSERVER=1 npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts'
```
