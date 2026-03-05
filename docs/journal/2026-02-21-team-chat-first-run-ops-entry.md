# Team Chat-First Run Ops Entry

## Background

Team workbench still exposed `Create Run` in the primary run panel, which kept the UX run-first.
For the chat-first direction, manual run creation should remain available but moved to Debug tools.

## Scope

- `TeamRunPanel` no longer renders create-run input/JSON controls.
- `TeamRunPanel` focuses on:
  - run list browsing
  - run status filtering
  - run refresh/load-more
  - active-run selection
- `Debug -> Run Ops` now contains both:
  - `Create Run` controls (`context_id`, JSON input, quick JSON actions, keyboard shortcut)
  - `Load Existing Run` by `run_id`

## Key Changes

1. Simplified `TeamRunPanel` surface to remove create-run controls and message users to Debug entry.
2. Added run-input validation in `TeamPage` Debug run-ops flow to keep safe JSON submission behavior.
3. Updated Team panel unit tests to align with the new primary/Debug split.

## Validation

- `cd web && npm run -s test -- src/pages/team_panels.test.tsx`
- `cd web && npm run -s test -- src/pages/team_page.runs.test.ts`
- `cd web && npm run -s build`

All commands passed locally during this change.

## Follow-up

- Continue chat-first flow by making leader negotiation/task conversation the explicit first action in Team UX copy and interaction hints.
