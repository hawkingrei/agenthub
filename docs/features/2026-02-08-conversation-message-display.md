# Conversation Message Display

## Background
Automatic collapsing of older conversation items can hide full AI replies in the
conversation view. This is confusing when users expect to read full responses
without expanding each entry.

## Scope
- Keep auto-collapse for thinking, plan, and tool-call blocks.
- Always show full text for user and agent messages.

## Decisions
- Remove auto-collapse rendering paths for `agent_message` and `user_message`.
- Retain summary-based folding for non-message items only.

## Validation
- Manual: scroll a long conversation and confirm AI replies render in full.
