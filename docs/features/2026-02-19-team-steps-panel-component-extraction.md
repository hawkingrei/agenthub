# Team Steps Panel Component Extraction

## Summary

Extract Team `steps` tab UI from `web/src/pages/team_page.tsx` into `TeamStepsPanel` while preserving step submit/action behavior.

## Background

With sidebar, run, overview, events, mailbox, and member-console already extracted, `steps` remained the last large tab-specific inline render block in `team_page.tsx`.

## Scope

- `web/src/pages/team_page.tsx`
- `web/src/pages/team_steps_panel.tsx`
- `docs/todo.md`

## Key Decisions

1. Keep mutation logic in `TeamPage` (`onSubmitStep`, `onApplyStepAction`, `refreshSteps`).
2. Keep `TeamStepsPanel` callback-driven and stateless with respect to run/session data.
3. Preserve all existing step-action input branches (`start`, `complete`, `fail`, `input_required`, `resume`) and disabled-state semantics.

## Validation

Executed (2026-02-19):

```bash
npm --prefix web run lint
npm --prefix web run test -- src/pages/team_page.runs.test.ts
npm --prefix web run build
/bin/zsh -lc 'set -euo pipefail; npm --prefix web run dev -- --host 127.0.0.1 --port 5173 --strictPort >/tmp/agenthub-vite.log 2>&1 & VITE_PID=$!; trap "kill $VITE_PID 2>/dev/null || true" EXIT; for i in {1..30}; do curl -sf http://127.0.0.1:5173 >/dev/null && break; sleep 1; done; PLAYWRIGHT_NO_WEBSERVER=1 npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts'
```
