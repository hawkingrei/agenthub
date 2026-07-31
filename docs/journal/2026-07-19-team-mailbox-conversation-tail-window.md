# Team Mailbox Conversation Tail Window

## Summary

Team mailbox conversations now use a bounded recent tail window while the conversation is pinned to the bottom. The mailbox panel only builds row presentation for the visible window, preserves a top spacer for hidden history, and restores the full conversation when the caller reports that the operator has scrolled away from the bottom.

## Background

The mailbox conversation already had row-level memoization, but long histories still mapped every mailbox message into row presentation before render. That kept hidden history in the expensive path for payload text resolution, mention HTML generation, and actor label formatting.

## Scope

- Windowed `conversationMessages` before building `MailboxConversationRow` values.
- Added a stable `data-team-mailbox-message-id` row hook for focused rendering tests.
- Added a top spacer when the pinned tail window hides older mailbox messages.
- Kept `Accept visible pending` scoped to the rows that are actually rendered in the visible window.
- Stabilized the open reply obligation fallback array so mailbox obligation rows do not rebuild from
  a new empty dependency on every render when the snapshot has no open obligations.

## Key Decisions

- Reuse the Team conversation viewport helper instead of adding a mailbox-specific list implementation.
- Keep routing, triage, accept, escalation, and mailbox transport behavior unchanged.
- Treat `chatStickToBottom=false` as the caller-owned signal that the operator detached from the tail; in that state the mailbox panel restores the full source list.

## Validation

Targeted checks for this slice:

```bash
cd web && npm exec vitest -- run src/pages/team_panels.test.tsx
cd web && npm exec tsc -- --noEmit
cd web && npm run lint
```

## Follow-Ups

- The broader frontend performance TODO still needs browser/profiler evidence before broad Team/ACP page-level responsiveness claims can be closed.
