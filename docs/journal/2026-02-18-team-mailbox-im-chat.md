# Team Mailbox IM Conversation Flow

## Summary

Upgrade Team Workbench mailbox UX from JSON-first operations to an IM-like flow:
select a member, view conversation history for that actor pair, and send quick chat messages.

## Background

Team mailbox APIs already support structured actor-to-actor messaging, but the UI required
manual `from_actor_id` / `to_actor_id` / payload JSON editing for common collaboration loops.
This made leader-to-worker coordination slower than needed during active runs.

## Scope

- `web/src/pages/team_page.tsx`
- `web/src/pages/team_page.runs.test.ts`
- `web/tests/e2e/team_page.e2e.ts`
- `web/src/styles.css`
- `docs/todo.md`

## Key Decisions

1. Add IM conversation helpers:
   - actor-pair resolution from `leader_member_id` + selected member,
   - merge recent mailbox records with inbox records,
   - filter bi-directional conversation by actor pair,
   - quick payload builder for `chat_message`,
   - stable conversation key + unread counting + seen watermark tracking.
2. Keep advanced JSON mailbox controls, but move them under a collapsible section.
3. Default member-card click action in run overview to open mailbox chat directly.
4. Add conversation auto-follow behavior:
   - when user is near bottom, new messages auto-scroll and mark as seen;
   - when user scrolls up, auto-follow pauses and unread can accumulate.
5. Use conversation-focused polling while in Mailbox tab:
   - auto-refresh fetches mailbox snapshot + active inbox only;
   - avoid refreshing full run events each tick in chat mode.
6. Keep backend API contract unchanged (`/api/teams/runs/:run_id/messages/send|inbox|ack`).

## Validation

Executed (2026-02-18):

```bash
npm --prefix web run test -- src/pages/team_page.runs.test.ts
/bin/zsh -lc 'set -euo pipefail; npm --prefix web run dev -- --host 127.0.0.1 --port 5173 --strictPort >/tmp/agenthub-vite.log 2>&1 & VITE_PID=$!; trap "kill $VITE_PID 2>/dev/null || true" EXIT; for i in {1..30}; do curl -sf http://127.0.0.1:5173 >/dev/null && break; sleep 1; done; PLAYWRIGHT_NO_WEBSERVER=1 npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts -g "team mailbox IM mode supports conversation focus"'
/bin/zsh -lc 'set -euo pipefail; npm --prefix web run dev -- --host 127.0.0.1 --port 5173 --strictPort >/tmp/agenthub-vite.log 2>&1 & VITE_PID=$!; trap "kill $VITE_PID 2>/dev/null || true" EXIT; for i in {1..30}; do curl -sf http://127.0.0.1:5173 >/dev/null && break; sleep 1; done; PLAYWRIGHT_NO_WEBSERVER=1 npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts'
npm --prefix web run build
```
