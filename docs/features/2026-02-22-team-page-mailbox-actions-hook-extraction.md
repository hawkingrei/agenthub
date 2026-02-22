# Team Page Mailbox Actions Hook Extraction

## Background

`web/src/pages/team_page.tsx` still contained raw mailbox action callbacks, mixing input validation, API dispatch, and refresh branching logic in the page component.

The remaining high-noise callbacks were:

- `onSendChatMessage`
- `onSendMessage`
- `onRefreshInbox`
- `onAckMessage`

## Scope

This change extracts mailbox action callbacks into a dedicated hook:

- Added `web/src/pages/team/use_team_mailbox_actions.ts`
- Added focused tests in `web/src/pages/team/use_team_mailbox_actions.test.tsx`
- Updated `web/src/pages/team_page.tsx` to consume the hook and removed inline mailbox callback block

## Key Decisions

1. Keep token-bound mailbox API calls in one memoized client.

- `useTeamMailboxActions` now owns `sendTeamRunMessage` / `ackTeamRunMessage` dispatch paths.

2. Preserve tab-branch behavior exactly.

- When `tab === "mailbox"`, raw send follows snapshot + optional inbox refresh.
- Otherwise, raw send follows events + snapshot refresh.
- Ack keeps mailbox/non-mailbox branch semantics.

3. Keep payload parsing close to dispatch.

- `route` and raw payload parsing remain in the mailbox-action layer with explicit errors.

## Validation

Executed locally:

- `npm run test -- use_team_mailbox_actions`
- `npm run test -- src/pages/team`
- `npm run lint`
- `npm run build`

All commands passed.

## Follow-up

- Continue extracting remaining view refresh helpers (`onRefreshMemberConsole`, `onRefreshOverviewSnapshot`, `onRefreshEventsPanel`) into small action hooks if needed.
