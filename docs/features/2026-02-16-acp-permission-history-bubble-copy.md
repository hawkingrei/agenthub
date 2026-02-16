# ACP Permission History Bubble and Copy Actions

## Summary

Enhance ACP Debug permission history with two interaction capabilities:

1. Click a permission history record to expand and render the original
   permission payload in ACP bubble style.
2. Provide a copy action that writes structured permission record content to
   clipboard.

## Background

Permission history in Debug only displayed shallow status metadata, which made
it hard to inspect the original permission context and replay/debug decisions.
Users also needed a quick way to copy permission evidence for issue reports and
reviews.

## Scope

- `web/src/components/acp_debug.tsx`
- `web/src/styles.css`
- `web/src/acp_debug_permissions.test.ts`
- `docs/todo.md`

## Key Decisions

1. Reuse existing ACP bubble visual language (`acp-bubble tool_call`) for
   permission detail expansion to keep UX consistent with conversation view.
2. Keep list collapsed by default and expand on click for readability in long
   histories.
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

- Permission history rows can be expanded to show ACP bubble-like detail.
- Copy button updates clipboard with structured JSON payload.
- Frontend tests and build pass.

## Follow-ups

- Add UI interaction test for expand/copy behavior in ACP Debug tab.
- Consider persisting expanded row state per permission ID across tab switches.
