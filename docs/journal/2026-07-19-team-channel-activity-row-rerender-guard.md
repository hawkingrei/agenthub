# Team Channel Activity Row Rerender Guard

## Summary

The Team channel conversation pane now renders visible activity rows through a memoized row component with an explicit comparator. Composer draft changes, mention picker state, and parent-only pane updates no longer need to rebuild unchanged visible channel rows.

## Background

The Team channel timeline already windows long histories before rich-text rendering. The remaining avoidable cost was row-level reconciliation: the visible activity row JSX lived inline in `TeamTaskPanel`, so parent-local state changes could still recreate every visible row even when the row data and visible controls did not change.

## Scope

- Added explicit `TeamTaskActivityItem`, `TeamTaskActivityRowModel`, and `TeamTaskActivityRowProps` shapes for visible channel rows.
- Extracted the inline channel row JSX into `TeamTaskActivityRow`, wrapped with `React.memo`.
- Added `areTeamTaskActivityRowPropsEqual` to compare visible row inputs, including active thread root state, read receipts, developer-details expansion state, permission card payload/record/busy/error state, author labels, avatar identity, markdown rendering, mention callbacks, and row registration callbacks.
- Preserved the existing tail-window behavior, jump-to-message refs, thread affordance, developer details, permission review cards, read receipt hover card, and rich-text rendering.

## Key Decisions

- Keep this as a row-level guard rather than introducing virtualization. The existing tail window remains the canonical bound for normal histories; virtualization or more advanced stick-to-bottom behavior is still a separate decision for extremely long histories.
- Compare permission records and live member state semantically so polling refreshes that preserve visible values do not force row updates.
- Keep mention-aware markdown rendering as a visible input. Display-name map changes must still refresh rendered message bodies.

## Validation

```bash
cd web && npm exec vitest -- run src/pages/team_panels.test.tsx
cd web && npm run build
git diff --check
```

## Follow-Ups

- The broader frontend performance TODO remains open for mailbox and task-board list surfaces, cross-page rerender audits, and explicit extremely long history behavior.
