# Team UI State Reducer Phase 4 (Create Team Wizard + Forge)

## Summary

Complete reducer migration for Team create flow state:
- mission brief inputs (`newTeamName`, `newTeamDescription`)
- team spec controls (`useSpecOverride`, `newTeamSpec`)
- modal/stage controls (`showCreateTeamModal`, `createTeamStage`)
- leader/worker draft controls (`leader*`, `workers`, `teamForgeAgentIds`)
- forge form controls (`forgeAgentBindTarget`, `showForgeAgentForm`, `forgeAgent*`)

## Background

After phases 1-3, Team page still kept create-team wizard state in many standalone `useState` atoms. This left wizard transitions, worker mutations, and forge bind logic split across independent setters. Phase 4 migrates these fields into `TeamCreateState` reducer, while preserving existing interaction behavior.

## Scope

- `web/src/pages/team_page.tsx`
- `docs/todo.md`

## Key Decisions

1. Introduce `TeamCreateState` + `TeamCreateAction` (`patch`) and centralize create-team state updates through reducer dispatch.
2. Keep existing handler call sites readable by exposing compatibility wrappers (`setWorkers`, `setLeaderSkills`, `setCreateTeamStage`, `setTeamForgeAgentIds`) that support both direct values and updater functions.
3. Use lazy reducer initialization (`createInitialTeamCreateState`) to avoid top-level initialization order issues with default prompts.

## Validation

Executed (2026-02-19):

```bash
npm --prefix web run lint
npm --prefix web run test -- src/pages/team_page.runs.test.ts
npm --prefix web run build
/bin/zsh -lc 'set -euo pipefail; npm --prefix web run dev -- --host 127.0.0.1 --port 5173 --strictPort >/tmp/agenthub-web-dev.log 2>&1 & DEV_PID=$!; trap "kill $DEV_PID >/dev/null 2>&1 || true" EXIT; for i in {1..60}; do curl -sSf http://127.0.0.1:5173 >/dev/null && break; sleep 1; done; PLAYWRIGHT_NO_WEBSERVER=1 npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts'
```
