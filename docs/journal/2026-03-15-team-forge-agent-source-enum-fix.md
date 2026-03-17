# Team Forge Agent Source Enum Fix

## Context

The Team `Add Agent` flow still sent the stale frontend source value `team_setup` when creating a forged agent. The backend agent API now only accepts `manual` or `team_forge`, so the request failed with:

- `source must be one of: manual, team_forge`

## Changes

- replaced the Team `Add Agent` request payload source with the backend-supported `team_forge` enum in `web/src/pages/team_page.tsx`
- centralized the allowed frontend agent source constants in `web/src/api.ts` so future callers do not hand-type source strings

## Validation

- `cd web && npm run lint -- src/api.ts src/pages/team_page.tsx`
- `cd web && npm run build`

## Chrome MCP Notes

Baseline:

- reproduced the team workbench flow against `agenthub.hawkingrei.com`
- confirmed the failure reason matched the backend agent source enum validation

Post-fix:

- no visible UI change; regression focus is the Team `Add Agent` request payload path in the rebuilt frontend
