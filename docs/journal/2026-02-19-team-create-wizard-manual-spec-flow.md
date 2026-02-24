# Team Create Wizard Manual Spec Flow

## Background

`Create Team` wizard supported manual spec override, but the toggle was only exposed in the
final stage (`Launch Team`). The stage gate required leader selection first, which made
manual-spec-first flows cumbersome and effectively blocked users from jumping directly to JSON
authoring.

## Scope

- `web/src/pages/team_page.tsx`
- `web/tests/e2e/team_page.e2e.ts`

## Key Decisions

1. Expose manual spec mode in stage 1 (`Mission Brief`) so users can choose flow early.
2. In manual spec mode:
   - treat stage 2 (`Leader Forge`) and stage 3 (`Recruit Workers`) as ready;
   - keep those stages accessible, but allow direct progression to stage 4;
   - `Next Stage` from stage 1 jumps directly to `Launch Team`.
3. Keep existing role/duplicate checks unchanged for non-manual flow.
4. Add Playwright coverage to ensure manual mode can create a team without forged leader/worker agents.

## Validation Evidence (2026-02-19)

- Command:
  - `/bin/zsh -lc 'set -e; PLAYWRIGHT_PORT=4174 npm run dev -- --host 127.0.0.1 --port 4174 --strictPort >/tmp/agenthub_vite_4174.log 2>&1 & dev_pid=$!; sleep 2; PLAYWRIGHT_NO_WEBSERVER=1 PLAYWRIGHT_PORT=4174 npm run e2e -- --grep "team forge modal creates team with leader/worker presets|team forge manual spec mode skips leader/worker stages" tests/e2e/team_page.e2e.ts; rc=$?; kill $dev_pid >/dev/null 2>&1 || true; wait $dev_pid >/dev/null 2>&1 || true; exit $rc'`
- Result:
  - `team forge modal creates team with leader/worker presets` passed.
  - `team forge manual spec mode skips leader/worker stages` passed.

## Notes

- This update improves page logic while preserving existing default guided flow semantics.
- Existing non-manual wizard behavior remains backward compatible.
