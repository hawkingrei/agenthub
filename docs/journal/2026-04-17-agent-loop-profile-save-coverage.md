# Agent Loop Profile Save Coverage

## Summary

Added focused Team page coverage for the operator-controlled `agent_loop` profile
save path so Team member profile edits stay successful even when the follow-up
`/api/agents/:id/agent_loop` call fails after the Team spec update succeeds.

## Changed

- `web/src/pages/team_page.agent_loop.test.tsx`
  - added a focused `TeamPage` regression that drives the Team member profile
    save flow through the real `onSaveTeamMemberProfile` logic
  - asserted that Team spec persistence still succeeds before the best-effort
    agent-loop API update
  - asserted that a failed loop update surfaces the warning banner instead of
    blocking the whole profile save

## Why

The phase-1 `agent_loop` contract is intentionally best-effort from the Team
member profile editor: Team-owned prompt/profile changes should persist first,
and loop watchdog reconfiguration should warn without failing the entire edit
transaction if the runtime-side update cannot be applied.

## Validation

- `cd web && npm run test -- vite.config.test.ts src/pages/team_page.agent_loop.test.tsx src/pages/team_page.smoke.test.tsx`
