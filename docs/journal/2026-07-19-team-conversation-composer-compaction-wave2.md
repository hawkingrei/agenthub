# Team Conversation Composer Compaction Wave 2

## Summary

Compacted a Team conversation/composer UI journal cluster into canonical feature specs. Stable
rules for chat-style composer behavior, duplicate-send protection, visible payload filtering,
conversation-scope resilience, message-row rhythm, neutral bubbles, chat-native markdown, and
shared channel/thread/ACP composer language now live in `docs/features/team-channels-threads.md`
and `docs/features/frontend-design.md`.

## Background

Several April and May notes recorded Team conversation polish, channel send feedback, and shared
composer behavior. The rollout evidence remains useful, but the stable rules should be discoverable
from feature specs instead of spread across date-based notes.

## Scope

This compaction pass covers:

- `2026-04-03-team-channel-conversation-alignment.md`
- `2026-04-05-team-channel-send-feedback-and-visibility.md`
- `2026-04-05-team-conversation-selection-resilience.md`
- `2026-04-24-team-conversation-slock-polish.md`

It updates only documentation. It does not change web runtime behavior.

## Key Decisions

- Team channel, thread, and embedded ACP conversations should read as one chat system.
- Channel/thread composer task intent remains upstream of coordinator/runtime task creation and
  must not bypass the agent-only canonical task materialization path.
- Team channel and thread composers should provide chat-style send behavior with page-local
  duplicate-send protection and authoritative persisted idempotency.
- Human streams should render visible chat messages and explicit permission-review cards, not raw
  task notes or ACP/debug payload JSON.
- Conversation scope changes should clear stale visible messages before rendering the next target.
- Chat markdown should use stable semantic structure classes so rich text remains chat-native.

## Validation

Focused checks for this documentation slice:

```bash
git diff --check
```

## Follow-Ups

- Continue compaction for remaining ACP-heavy UI journals.
- Keep deployed browser validation follow-ups separate from documentation compaction.
