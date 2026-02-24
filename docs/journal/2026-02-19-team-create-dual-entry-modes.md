# Team Create Dual Entry Modes

## Background

`Create Team` previously mixed two flows inside one wizard entry:

- guided setup via `Leader Forge` / `Recruit Workers`;
- manual JSON authoring via in-wizard `Manual spec mode` toggles.

That made the advanced path discoverable only after entering the wizard and added mode-switch
state inside stages.

## Scope

- `web/src/pages/team_sidebar.tsx`
- `web/src/pages/team_page.tsx`
- `web/src/styles.css`
- `web/src/pages/team_panels.test.tsx`
- `web/tests/e2e/team_page.e2e.ts`
- `docs/todo.md`

## Key Decisions

1. Add explicit Team Forge entry points in sidebar:
   - `Guided Wizard`
   - `Manual Spec`
2. Keep one modal, but let entry mode decide behavior:
   - guided entry keeps stage-by-stage forge flow;
   - manual entry starts from `Mission Brief` and jumps directly to `Launch Team` on next.
3. Remove in-wizard manual mode toggles from:
   - `Mission Brief` stage;
   - `Launch Team` stage.
4. Keep existing guided validation contracts unchanged:
   - leader must be selected from forged agents;
   - duplicate leader/worker assignment is blocked.

## Validation Evidence (2026-02-19)

- Passed:
  - `npm --prefix web run test -- src/pages/team_panels.test.tsx`
  - `npm --prefix web run build`
- Needs follow-up:
  - `npm --prefix web run e2e -- --grep "team forge modal creates team with leader/worker presets|team forge manual spec mode skips leader/worker stages" web/tests/e2e/team_page.e2e.ts`
  - Result: timed out waiting Playwright `config.webServer` startup in current environment.

## Notes

- This change intentionally removes runtime mode switching inside the wizard and shifts it to
  explicit entry selection.
- Existing manual-spec E2E scenarios were updated to use the `Manual Spec` entry instead of a
  Mission Brief checkbox.
