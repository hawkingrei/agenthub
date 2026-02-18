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

Executed:

```bash
npm --prefix web run test -- src/pages/team_page.runs.test.ts
npm --prefix web run lint
npm --prefix web run build
```

Manual checks to run in browser:

1. Open `/teams`, select a run, click a member card, and verify tab switches to `Mailbox` with selected conversation pair.
2. Send `Send Chat` message and verify it appears in conversation immediately.
3. Scroll chat upward, trigger new mailbox messages, and verify `auto_follow=off` with unread counter increase.
4. Scroll back to bottom and verify unread clears for active conversation.
5. Verify `Ack` from conversation list updates status and mailbox summary counters.
6. Expand `Advanced mailbox controls` and verify legacy JSON send/inbox still works.
