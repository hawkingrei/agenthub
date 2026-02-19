# Team UI State Reducer Phase 3 (Mailbox + Member Conversation)

## Summary

Extend Team page reducer migration to include mailbox/member conversation state:
- mailbox send controls (`msgFromActorId`, `msgToActorId`, `msgChannel`, `msgTransport`, `msgRoute`, `msgTemplate`, `msgPayload`, `msgIdempotencyKey`)
- chat controls (`chatDraft`, `chatStickToBottom`, `chatSeenByConversation`)
- inbox filters and records (`inboxActorId`, `inboxLimit`, `inboxAfterId`, `inboxIncludeDelivered`, `inbox`)
- member focus (`selectedMemberId`)

## Background

Phase 1 and phase 2 moved tab/run/step state into reducers, but mailbox/member conversation still used many `useState` atoms with cross-effect coupling (`snapshot`, `inbox`, unread calculation, auto-follow). This phase centralizes mailbox/member state to make update paths explicit and reduce local state drift risk.

## Scope

- `web/src/pages/team_page.tsx`
- `docs/todo.md`

## Key Decisions

1. Introduce `TeamMailboxState` + `TeamMailboxAction` with three actions:
   - `patch` for direct field updates
   - `mark_conversation_seen` for monotonic unread watermark updates
   - `reset_chat_seen` for run switch cleanup
2. Preserve existing call sites by exposing callback wrappers (`setInboxActorId`, `setChatDraft`, `setSelectedMemberId`, etc.) over reducer dispatch.
3. Keep create-team draft state migration out of this phase to contain blast radius.

## Validation

Executed (2026-02-19):

```bash
npm --prefix web run lint
npm --prefix web run test -- src/pages/team_page.runs.test.ts
npm --prefix web run build
/bin/zsh -lc 'set -euo pipefail; npm --prefix web run dev -- --host 127.0.0.1 --port 5173 --strictPort >/tmp/agenthub-web-dev.log 2>&1 & DEV_PID=$!; trap "kill $DEV_PID >/dev/null 2>&1 || true" EXIT; for i in {1..60}; do curl -sSf http://127.0.0.1:5173 >/dev/null && break; sleep 1; done; PLAYWRIGHT_NO_WEBSERVER=1 npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts'
```
