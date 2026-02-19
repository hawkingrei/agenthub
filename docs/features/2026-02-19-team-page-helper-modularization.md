# Team Page Helper Modularization

## Background

`web/src/pages/team_page.tsx` accumulated both UI composition and many pure helper utilities.
That made review and maintenance expensive, especially for run/mailbox/member helper logic
already covered by dedicated unit tests.

## Scope

- Extract pure Team helper logic into smaller modules under `web/src/pages/team/`.
- Keep `TeamPage` UI behavior unchanged.
- Preserve existing test import surface from `team_page.tsx` via re-export.

## Modules

- `web/src/pages/team/run_helpers.ts`
  - run list merge, status filter, preview selection.
- `web/src/pages/team/mailbox_helpers.ts`
  - mailbox actor resolution, message merge/conversation selection, payload templates.
- `web/src/pages/team/member_helpers.ts`
  - team member/spec parsing, lifecycle summary/live-state shaping, skill normalization, draft helpers.
- `web/src/pages/team/state.ts`
  - Team page reducer state types/constants/defaults and reducer functions.
- `web/src/pages/team/create_helpers.ts`
  - Team spec build and create-wizard parsing/stage helper logic.
- `web/src/pages/team/page_helpers.ts`
  - shared page-level pure helpers for run/event upsert, agent label formatting, and display formatting.

## Compatibility Decision

`web/src/pages/team_page.tsx` re-exports helper symbols so existing tests (for example
`src/pages/team_page.runs.test.ts`) do not require import path changes.

## Validation

- `npm --prefix web run lint -- src/pages/team_page.tsx \"src/pages/team/*.ts\"`
- `npm --prefix web run test -- src/pages/team_page.runs.test.ts`
- `npm --prefix web run test -- src/pages/team_panels.test.tsx`
- `npm --prefix web run build`
