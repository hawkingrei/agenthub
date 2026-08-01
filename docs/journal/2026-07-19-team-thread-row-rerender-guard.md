# Team Thread Row Rerender Guard

## Summary

The Team thread pane now renders root and reply messages through a memoized row component with an explicit comparator. Composer draft changes, mention picker state, and other parent-only thread-pane updates no longer need to rebuild every visible thread message row when the row inputs are unchanged.

## Background

The frontend performance hardening TODO covers Team and ACP-heavy pages. ACP conversation rows already gained a row-local focus guard. Team thread panes still held message row JSX inline inside the parent pane, so long reply threads could pay unnecessary row render cost when only composer-local state changed.

## Scope

- Extracted the root and reply message row markup in `web/src/pages/team/team_thread_pane.tsx` into a memoized `TeamThreadMessageRow`.
- Added `areTeamThreadMessageRowPropsEqual` as a narrow comparator for message identity, author, timestamp label, text, avatar seed, original-row state, missing-text fallback, and mention-aware HTML rendering.
- Preserved existing root empty-text fallback, original badge, unknown-author avatar seed behavior, and reply rich-text rendering.

## Key Decisions

- Keep this slice row-local instead of introducing virtualization. The current change removes avoidable parent-state rerenders without changing scroll mechanics or long-history loading behavior.
- Treat `renderSanitizedHtml` identity as row-visible input. Mention display-name map changes must still refresh existing message bodies because rendered mention labels can change without message text changing.
- Export the comparator for focused unit coverage rather than relying on brittle render-count assertions.

## Validation

Targeted checks for this slice:

```bash
cd web && npm exec vitest -- run src/pages/team/team_thread_pane.test.tsx
git diff --check
```

## Follow-Ups

- The broader frontend performance TODO remains open for cross-page Team list audits, extremely long history behavior, and virtualization or stick-to-bottom evaluation where needed.
