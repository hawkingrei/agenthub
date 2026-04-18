# Team Page Shell Split

- Date: 2026-04-18

## Summary

Extract the highest-risk render scaffolding from `team_page.tsx` so future workspace-shell changes
do not keep editing one deeply nested JSX tree.

## Changes

- add `web/src/pages/team/team_page_shell.tsx` for the outer Team page frame:
  - header
  - notice/error slots
  - selector vs detail layout split
  - sidebar / workbench placement
- add `web/src/pages/team/team_page_modals.tsx` for Team modal hosting:
  - create team
  - forge member agent
  - edit member profile
- keep `team_page.tsx` as the stateful orchestration layer and move the outer render shell /
  modal mounting into dedicated components
- export `TeamModalChrome` from `team_management_modals.tsx` so the new modal host can reuse the
  existing modal contract without re-defining it

## Validation

- `make build-web`
- `cd web && npm run test -- vite.config.test.ts src/pages/team_page.smoke.test.tsx src/pages/team_panels.test.tsx src/pages/team/team_page_header.test.tsx`

## MCP Check

- baseline shell review used `https://agenthub.hawkingrei.com/workspace/teams/276a2682-9ce7-4af5-aa6c-f12575d13c37`
- post-edit regression should be checked on the local build because these shell refactors are not
  deployed yet
