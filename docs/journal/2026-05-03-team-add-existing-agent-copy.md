# Team Add Existing Agent Copy

## Summary

Implemented the first Team agent adoption slice by adding a dedicated `Copy Existing Agent` path
next to the existing `Create New Agent` flow.

The default Team add-agent experience now has two explicit entry points:

- `Create New Agent`
- `Copy Existing Agent`

## Background

The Team create/add-agent simplification work had already narrowed the default flow to fixed-role
creation:

- first Team member -> coordinator
- later Team members -> worker

That removed role-switch ceremony, but the product still assumed that every Team member would be
forged from scratch inside the Team flow. The new adoption spec added a clearer contract for
existing-agent reuse, with `copy` as the first supported mode and `move` deferred.

## Scope

- add a Team UI entry point for copying an existing agent into the current Team
- keep the existing `Create New Agent` Team forge path intact
- preserve coordinator-first Team role assignment with no new role toggle
- document the resulting Team create/add-agent contract

## Key Decisions

- `Create New Agent` remains the primary create-from-scratch path
- `Copy Existing Agent` is a separate explicit path, not a mode hidden inside the forge form
- `copy` creates a new Team-owned agent record:
  - source agent remains unchanged
  - copied Team member gets a new agent id
  - copied Team member uses copied runtime settings plus fresh Team membership
- default `copy` is configuration-only:
  - workspace path/worktree settings may carry over
  - workspace contents, runtime history, and memory do not
- the fixed-role Team contract still applies:
  - empty Team copy -> coordinator
  - later copy -> worker
- `move existing agent to Team` remains out of scope for this slice

## Validation

```bash
cd web && npm exec vitest -- run src/pages/team/team_management_modals.test.tsx src/pages/team/forge_helpers.test.ts src/pages/team/use_team_management_actions.test.tsx src/pages/team_setup_panel.test.tsx src/pages/team_panels.test.tsx
cd web && npm exec tsc -- --noEmit
cd web && npm run build
cd web && npm run lint
```

## Follow-Ups

- add browser-level coverage for `Copy Existing Agent` in the Team flow
- consider whether copied Team members should expose source provenance in the visible Team UI
- evaluate future opt-in support for:
  - workspace-content copy
  - memory/context seeding
- evaluate a later `move` rollout only after ownership/runtime constraints are explicit
