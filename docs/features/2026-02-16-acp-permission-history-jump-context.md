# ACP Permission History Jump With Context

## Summary

Update ACP Debug permission history row interaction from inline detail expansion
to conversation navigation:

1. Clicking a permission history row switches to Conversation tab.
2. The UI jumps to the linked tool-call bubble by `tool_call_id`.
3. The bubble is centered to preserve nearby context and briefly highlighted.

## Background

Inline rendering in Debug showed the payload, but did not help users inspect the
original execution timeline. For troubleshooting and review, users need to land
at the exact historical location with surrounding messages/tool calls intact.

## Scope

- `web/src/hooks/use_acp_conversation.ts`
- `web/src/components/acp_conversation.tsx`
- `web/src/components/acp_debug.tsx`
- `web/src/app.tsx`
- `web/src/styles.css`
- `web/src/hooks/use_acp_conversation.test.ts`
- `docs/todo.md`

## Key Decisions

1. Use `tool_call_id` as the stable linkage between permission history records
   and conversation bubbles.
2. Keep virtualization enabled; jump first uses estimated row offset, then
   performs an exact DOM centering pass if target row is rendered.
3. Keep copy button behavior unchanged to preserve incident-report workflow.
4. Add temporary visual focus style to reduce user disorientation after jump.

## Validation

```bash
cd web
npm run test -- use_acp_conversation.test.ts acp_debug_permissions.test.ts app.permission_scope.test.ts
npm run build
```

Expected outcomes:

- Clicking a permission row in Debug switches to Conversation.
- The linked tool-call bubble is visible and centered with surrounding context.
- The target bubble gets a temporary highlight.
- Copy action still works and tests/build pass.

## Follow-ups

- Add component-level UI interaction tests for Debug row click and jump
  behavior under virtualized long conversations.
- Consider fallback navigation by timestamp/title when `tool_call_id` is absent.
