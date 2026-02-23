# 2026-02-24 Team Start Button Restore

## Context

Users could not find a visible "Start Team" action in the Team workbench after run controls were moved toward Debug Run Ops.

## Goal

Restore a clear primary-surface quick-start action without reintroducing full run-ops form controls into the main panel.

## Changes

1. Added quick action button in Team run panel:
   - `web/src/pages/team_run_panel.tsx`
   - new button: `Start Team`
   - behavior: triggers existing run creation handler (`onStartTeam`)
   - disabled when no selected team or while `create-run` is busy

2. Wired page-level action:
   - `web/src/pages/team_page.tsx`
   - pass `onCreateRun` into `TeamRunPanel` as `onStartTeam`

3. Updated panel tests:
   - `web/src/pages/team_panels.test.tsx`
   - assert quick-start button exists and callback is called
   - keep assertion that full `Create Run` form controls are not present in the primary surface

## Validation

- `npm --prefix web run test -- src/pages/team_panels.test.tsx`
- `npm --prefix web run test -- src/pages/team_page.runs.test.ts`
