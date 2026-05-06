# Team Create First-Agent Coordinator

## Summary

Narrowed the Team create contract so `Create Team` stays mission-first and the coordinator is
established when the first agent is added, not during the initial create modal itself. The default
add-agent modal now keeps role assignment fixed to the already-resolved Team state instead of
showing a coordinator/worker switch.

## Background

The earlier Team create spec had drifted toward forcing coordinator selection during Team
creation. That no longer matched the intended product path. The runtime and add-agent behavior
already expect the first added Team agent to be the coordinator, but the spec and UI copy still
overemphasized coordinator choice during Team shell creation.

## Scope

- update the canonical Team create spec
- align the active TODO wording
- make the create/add-agent UI explicitly state that the first agent becomes coordinator
- remove the default add-agent role-switch ceremony once the role is already determined
- lock the regression boundary with focused web tests

## Key Decisions

- `Create Team` remains a mission-only Team shell flow
- the first added agent becomes coordinator by default
- worker creation remains blocked until a coordinator already exists
- the add-agent surface should explain this contract directly instead of implying a later role
  cleanup step
- once the Team state already determines the role, the default add-agent modal should present a
  fixed role summary instead of an interactive toggle

## Validation

```bash
cd web && npm exec vitest -- run src/pages/team/team_management_modals.test.tsx src/pages/team/forge_helpers.test.ts src/pages/team/use_team_management_actions.test.tsx
cd web && npm exec tsc -- --noEmit
cd web && npm run build
```

## Follow-Ups

- keep simplifying the add-agent surface so it reads as participant setup rather than forge-stage
  ceremony, especially around how existing agents are picked or prefilled
- add browser-level Team create coverage once the current coordinator-first create-shell wording is
  fully reflected in the main Team flow

## 2026-05-06 Shell-First Follow-Up

`Create Team` now creates only the empty Team shell. After a successful create, the create modal
closes, the first-agent forge draft is cleared, and the operator lands on the Team detail page where
the normal `Add Agent` action owns first-agent setup.

This removes the remaining mismatch where Team creation still auto-opened the coordinator forge
modal even though the product contract had already moved coordinator setup into the first explicit
add-agent step.

Validation:

```bash
npm --prefix web run test -- src/pages/team/use_team_management_actions.test.tsx
npm --prefix web run lint
cd web && ./node_modules/.bin/tsc --noEmit
npm --prefix web run build
git diff --check
```
