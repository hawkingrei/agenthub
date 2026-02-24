# Team Forge Quest Flow UX Guardrails

## Background

The Team Forge modal already provided a staged flow, but users could still hit confusing states:

- advancing stages without completing required inputs,
- unclear duplicate-member conflicts when leader/worker selections collided,
- repetitive worker setup when many agents needed to be added.

This made the flow feel less game-like and more error-prone.

## Scope

- Add stage-guard navigation logic:
  - prevent forward stage jumps unless prerequisite stages are complete,
  - keep backward navigation available.
- Add a quest checklist panel in modal body with readiness indicators.
- Tighten `Next Stage` readiness:
  - stage 0 requires team name,
  - stage 1 requires valid leader agent,
  - stage 2 requires unique member assignments.
- Add worker setup helpers:
  - `Auto Fill Party` to append all remaining unassigned agents as workers,
  - `Resolve Duplicates` to auto-fix/clear duplicate worker assignments.
- Add inline guidance notes and action-bar blocking reason text for faster recovery.
- Extend Playwright Team E2E:
  - verify duplicate-assignment block on stage 2,
  - verify duplicate resolution re-enables progression.

## Key Decisions

- Keep backend payload schema unchanged; this is UX guidance and guardrail only.
- Keep manual JSON override path unchanged in final stage.
- Duplicate handling remains non-destructive:
  - first occurrence is preserved,
  - duplicates are reassigned to next available agent or cleared if none available.

## Validation

Run:

```bash
npm --prefix web run lint
HTTP_PROXY= HTTPS_PROXY= ALL_PROXY= NO_PROXY=127.0.0.1,localhost npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts --project=chromium
```
