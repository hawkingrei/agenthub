# Team Agent Entry Wording Unification

## Summary

- removed user-facing `Add Leader` / `Create Leader` wording from the Team setup flow
- standardized Team member entry actions on `Add Agent` / `Create Agent`
- reframed first-member setup copy around `first agent` / `primary agent` language while keeping the underlying leader/worker role model unchanged

## Implementation Notes

- `web/src/pages/team_page.tsx`
  - Team setup CTA always reads `Add Agent`
  - empty-team setup copy now refers to the first agent instead of the leader agent
  - setup checklist now uses `Create the first agent` and `Add more agents`
  - Team create modal note now says agents are added after the team exists
  - Team member forge modal now uses:
    - title: `Add Agent`
    - confirm action: `Create Agent`
    - first-member profile badge: `Primary Agent Profile`
- `web/src/pages/team/create_helpers.ts`
  - worker-first validation error now says `Create the first agent before adding more agents`
- `web/tests/e2e/team_page.e2e.ts`
  - updated setup-flow assertions and modal helpers to expect the unified agent wording

## Validation

- `cd web && npx vitest run src/pages/team/create_helpers.test.ts src/app.route_auth.test.ts src/pages/team_panels.test.tsx`
- `cd web && npm run lint -- src/pages/team_page.tsx src/pages/team/create_helpers.ts src/pages/team/create_helpers.test.ts tests/e2e/team_page.e2e.ts`
- `cd web && npm run build`

## Chrome DevTools MCP Notes

- deployed baseline on `https://agenthub.hawkingrei.com/teams/276a2682-9ce7-4af5-aa6c-f12575d13c37` still exposed:
  - `Add Leader`
  - `Create Leader`
  - `Create the leader`
  - `the leader agent`
  - `ADD LEADER AGENT` modal heading
- this follow-up change aligns those user-facing entry points with the goal-first `Add Agent` workflow.
