# Team Web Retention Caps

## Summary

- Added explicit client-side retention caps for Team shared conversation, Team run events, and Team member ACP events.
- Kept the cap at the state-merge helper layer so every refresh path shares the same bounded behavior.
- Left explicit older-history browsing semantics unchanged for `prepend` event paths; the cap applies to normal replace refresh windows, not user-driven older-page expansion.

## Details

- `web/src/pages/team/page_helpers.ts`
  - `TEAM_CONVERSATION_MESSAGE_RETENTION_LIMIT = 20`
  - `TEAM_RUN_EVENT_RETENTION_LIMIT = 100`
  - `TEAM_MEMBER_EVENT_RETENTION_LIMIT = 300`
  - `mergeConversationMessages(...)` now trims to the newest recent-20 shared-thread messages.
  - `upsertEventList(..., "replace")` now trims run events to the newest retained window after dedupe and sort.
  - `upsertAgentEventList(..., "replace")` now trims member event state to the newest retained window after merge.
- `web/src/pages/team/page_helpers.test.ts`
  - Added focused regression coverage for all three retention paths.

## Why Not Browser HTTP Cache

- The current problem is live in-memory state growth from merged event streams, not static asset delivery.
- Browser HTTP cache or Cache Storage is the wrong layer for mutable ordered event histories.
- If we later want persistence across refreshes, the right follow-up is a bounded browser-side state store (for example `localStorage` or `IndexedDB`) that hydrates a recent window, not an unbounded event log.

## Verification

- `cd web && npm run test -- src/pages/team/page_helpers.test.ts src/output_cache.test.ts`
- `make build-web`
