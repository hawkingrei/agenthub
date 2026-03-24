## Summary

Added member-scoped agent lifecycle controls to the Team workspace and aligned the Team ACP panel
with the standalone Agents ACP header so active thinking state is visible in both places.

## Why

Two Team workspace gaps were still slowing operators down:

- selected Team members did not expose the same direct `Start Agent`, `Stop Agent`, and
  `Delete Agent` controls that already existed on the main Agents page
- Team `Agent ACP` rendered ACP conversation content, including `agent_thought`, but it did not
  surface the same header-level status metadata and active `thinking Ns` affordance visible on the
  standalone Agents ACP view

## What Changed

- added `resolveTeamMemberAgentControlState(...)` to keep Team member lifecycle button gating in
  one place
- added `Start Agent`, `Stop Agent`, and `Delete Agent` actions to the selected Team member
  workspace menu
- wired those actions to the existing agent lifecycle APIs and refreshed Team runtime / agent cache
  after completion
- kept Team member deletion scoped to the underlying agent record only; the Team spec still retains
  the member until the profile is edited explicitly
- added a Team ACP header that mirrors standalone ACP status rendering:
  - member title
  - model tag
  - status badge
  - active `thinking Ns`
  - role pill
  - session pill in developer mode
- added focused tests for Team member lifecycle control derivation and Team ACP header rendering

## Validation

- `npm --prefix web run test -- --run src/pages/team_page.runs.test.ts src/pages/team_page.smoke.test.tsx src/pages/team_panels.test.tsx`
- `npm --prefix web run build`
- `git -c core.fsmonitor=false diff --check`

## Chrome Notes

- Remote baseline check against `https://agenthub.hawkingrei.com/teams/276a2682-9ce7-4af5-aa6c-f12575d13c37`
  succeeded after login. The deployed Team workspace menu currently exposes `Overview`, `Events`,
  `Steps`, and `Execution Mailbox`, but it does not yet include the new per-member `Start Agent`,
  `Stop Agent`, or `Delete Agent` actions.
- Local browser regression against `http://127.0.0.1:4174/` was not available at validation time
  because the local Team frontend route returned `ERR_CONNECTION_REFUSED`, so the unmerged UI change
  was validated via focused Vitest coverage plus production build output.
