# Team Task Board Card Rerender Guard

## Summary

The Team task board now renders kanban task cards through a memoized card component with an explicit comparator. Parent-only board state changes no longer need to rebuild unchanged visible task cards.

## Background

The frontend performance hardening TODO had already covered ACP conversation rows, Team thread rows, Team channel activity rows, and Team mailbox conversation rows. The remaining row-local list surface was the Team task board: card JSX still lived inline inside the lane render loop, so filter/detail state changes could recreate every visible task card.

## Scope

- Added `TeamTaskBoardCard`, wrapped with `React.memo`.
- Added `areTeamTaskBoardCardPropsEqual` to compare task card visible inputs: task identity, title, status, priority, assignee, created/updated labels, active state, developer-mode id visibility, priority badge text/class, and select callback.
- Kept task `context` out of the card comparator because the board card does not render it; detail and compile flows still read the selected task record from panel state.
- Preserved existing lane grouping, sorting, filter behavior, selected-card styling, developer id visibility, and task detail behavior.

## Key Decisions

- Keep this as a row-local guard instead of changing the board data model or introducing virtualization.
- Treat task metadata that can affect lane placement or card text as card-visible input, while ignoring task context clones that do not change board card output.
- Keep select callback identity in the comparator so card click behavior updates if parent ownership changes.

## Validation

```bash
cd web && npm exec vitest -- run src/pages/team_panels.test.tsx
cd web && npm run build
git diff --check
```

## Follow-Ups

- The broader frontend performance TODO remains open for cross-page rerender audits and explicit extremely long history behavior.
