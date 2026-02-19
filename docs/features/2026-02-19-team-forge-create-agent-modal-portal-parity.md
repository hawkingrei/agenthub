# Team Forge Create Agent Modal Portal Parity

## Background

Team Forge opened `CreateAgentModal` inside the Team wizard container (`withinPortal={false}`).
Nested modal stacking caused interaction issues in constrained viewports and diverged from the
main Agent create experience.

## Scope

- `web/src/pages/team_page.tsx`

## Key Decisions

1. Render Team Forge `CreateAgentModal` in portal mode so it behaves like the standard Agent
   create modal flow.
2. Keep Team wizard state ownership unchanged; only adjust modal rendering layer.

## Validation Evidence (2026-02-19)

- Command:
  - `cd web && npm run build`
- Result:
  - Web bundle build passed.

## Follow-up

- Re-run Team Forge Playwright suite in an environment that can launch Chromium successfully and
  verify no regressions for forge/bind flow.
