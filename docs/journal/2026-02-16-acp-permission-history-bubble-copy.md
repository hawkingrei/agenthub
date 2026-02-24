# ACP Permission History Bubble and Copy Actions

## Summary

Enhance ACP Debug permission history with two interaction capabilities:

1. Click a permission history record to jump to the corresponding Conversation
   tool-call bubble (preserving surrounding context).
2. Provide a copy action that writes structured permission record content to
   clipboard.

## Background

Permission history in Debug only displayed shallow status metadata, which made
it hard to locate the original tool call and replay/debug decisions. Users also
needed a quick way to copy permission evidence for issue reports and reviews.

## Scope

- `web/src/components/acp_debug.tsx`
- `web/src/styles.css`
- `web/src/acp_debug_permissions.test.ts`
- `docs/todo.md`

## Key Decisions

1. Reuse existing ACP bubble visual language (`acp-bubble tool_call`) for
   conversation rendering and make history rows act as navigation entries.
2. Keep Debug history compact; use row click for navigation instead of inline
   expansion to avoid duplicating long payload rendering inside Debug.
3. Copy action uses `navigator.clipboard` with DOM fallback to maximize browser
   compatibility.
4. Copy payload is structured JSON including IDs, status, options, timestamps,
   and tool-call payload for audit/debug handoff.

## Validation

```bash
cd web
npm run test -- acp_debug_permissions.test.ts app.permission_scope.test.ts
npm run build
```

Expected outcomes:

- Permission history row click navigates to linked Conversation tool call.
- Copy button updates clipboard with structured JSON payload.
- Frontend tests and build pass.

## Follow-ups

- Add UI interaction test for jump/copy behavior in ACP Debug tab.
- Consider adding fallback hint when tool call exists in history but is outside
  currently loaded conversation window.
