# ACP Tool Call Group Fold Animation

## Summary

Group consecutive ACP tool calls into a single conversation bubble, with a shared fold control and subtle entry/expand animations.

## Supersession

Stable ACP tool-call grouping, nested jump, and reduced-motion rules from this note now live in
`docs/features/acp-runtime.md#4-conversationdebug-surfaces` and
`docs/features/frontend-design.md#42-acp-heavy-output-visual-contract`. This journal remains the
rollout evidence for the group fold animation pass.

## Background

When an agent emits multiple tool calls in one response turn, rendering each tool call as an independent top-level bubble makes the timeline noisy and harder to scan. It also fragments fold behavior and weakens debug-to-conversation jump ergonomics.

## Scope

- `web/src/conversation.ts`
- `web/src/components/acp_conversation.tsx`
- `web/src/hooks/use_acp_conversation.ts`
- `web/src/conversation.test.ts`
- `web/src/acp_conversation_render.test.tsx`
- `web/src/hooks/use_acp_conversation.test.ts`
- `web/src/hooks/use_acp_conversation.interaction.test.tsx`
- `web/src/styles.css`
- `docs/todo.md`

## Key Decisions

1. Aggregate consecutive tool-call conversation items into a single `tool_call_group` item in `buildConversationMessages`.
2. Keep per-call details intact inside the group body and expose one shared collapse/expand interaction at group level.
3. Preserve debug jump semantics by keeping `data-tool-call-id` markers on nested grouped entries.
4. Treat grouped tool calls as a conversation focus target when any nested call matches the requested tool call id.
5. Add lightweight CSS motion for tool-call entry and grouped-body reveal, with `prefers-reduced-motion` fallback.
6. Hide debug-noise payload keys (`turn_id`, `process_id`, `source`, including normalized casing variants) from rendered tool payload cards.

## Validation

```bash
npm --prefix web run test -- \
  src/conversation.test.ts \
  src/acp_conversation_render.test.tsx \
  src/hooks/use_acp_conversation.test.ts \
  src/hooks/use_acp_conversation.interaction.test.tsx \
  src/acp_conversation.test.ts \
  src/acp_conversation.interaction.test.tsx

npm --prefix web run test
npm --prefix web run build
npm --prefix web run lint
```

Expected outcomes:

- Consecutive tool calls render under one shared `Tool Calls (N)` fold.
- Grouped folds can collapse/expand together while each nested call remains inspectable.
- Debug "jump to tool call" can locate grouped entries.
- Motion remains subtle and respects reduced-motion preferences.
