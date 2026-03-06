# 2026-03-06 Teams Subject Rail Layout

## Context

The earlier Team UI direction was reset to avoid repeating a broad shell rewrite. The next pass
needed to learn from Slock's composition without copying its skin:

- keep `/teams` only;
- keep current project code style;
- move from tab-first navigation toward subject-first navigation;
- keep backend Team semantics unchanged.

## Implementation

- reworked the Team sidebar into a subject rail:
  - team switcher button with expandable team picker;
  - `Human` section for shared conversation;
  - `Agents` section with leader/worker entries and per-agent secondary view buttons;
  - `Utilities` section for run-scoped operational tools.
- removed the old Team-mode switch strip and the large explanatory Team Forge prose block from the
  primary left rail.
- added a compact workspace context header in the main pane so the selected Team / Agent / Run
  context is visible without scanning multiple cards.
- kept the existing `tab` and `selectedMemberId` state model instead of introducing a new route or
  backend subject model.
- kept Team semantics intact:
  - no new `channel` concept;
  - no backend schema/API changes;
  - no change to task/conversation/mailbox ownership.
- reduced the top tab bar from a global flat tool list into a context-aware secondary tab bar:
  - agent-focused views only show `ACP`, `Console`, `Mailbox`;
  - team-level workspace keeps `Conversation`, `Runs`, `Overview`, `Events`, `Steps`, `Debug`.

## Validation

- `npm run lint`
- `npm run test -- team_panels.test.tsx`
- `npm run test -- team_page.runs.test.ts`
- `npm run build`

## Constraint

Chrome DevTools MCP was still unavailable during this change:

- `chrome-devtools/list_pages` -> `Transport closed`

So this pass includes code/test/build validation, but not the required MCP before/after visual
inspection yet. The follow-up verification TODO remains open until MCP transport is restored.
