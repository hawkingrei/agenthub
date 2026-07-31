# Team Mailbox Conversation Row Rerender Guard

## Summary

The Team mailbox conversation pane now renders mailbox message rows through a memoized row component with an explicit comparator. Chat draft changes, parent panel updates, and polling refreshes that preserve visible row data no longer need to rebuild unchanged mailbox conversation rows.

## Background

The frontend performance hardening TODO still covered mailbox list surfaces after the ACP, Team thread, and Team channel activity row guards landed. `TeamMailboxPanel` already precomputed conversation row presentation, but the `<li>` row JSX remained inline in the parent panel, so parent-local state changes still recreated every visible row.

## Scope

- Added `MailboxConversationMessageRow`, wrapped with `React.memo`.
- Added `areMailboxConversationMessageRowPropsEqual` to compare message identity/status/disposition fields, rendered payload, actor labels, timestamp label, acceptability, busy state, and row callbacks.
- Moved timestamp formatting into the row model so row comparison does not depend on a per-render `formatTs` call inside the row.
- Preserved existing mention-click routing, accept-button behavior, JSON fallback rendering, and empty-state behavior.

## Key Decisions

- Keep this as a row-local guard. Mailbox conversation history still uses the existing panel list behavior; virtualization remains a separate decision for extremely long histories.
- Compare mailbox message records semantically enough to catch delivery, disposition, thread-claim, linked-task, and payload changes while allowing object clones with unchanged visible state to skip row updates.
- Keep the row callback references in the comparator. If parent ownership handlers change, the row must refresh so click behavior stays current.

## Validation

```bash
cd web && npm exec vitest -- run src/pages/team_panels.test.tsx
cd web && npm run build
git diff --check
```

## Follow-Ups

- The broader frontend performance TODO remains open for task-board list surfaces, cross-page rerender audits, and explicit extremely long history behavior.
