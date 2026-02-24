# ACP Permission History Agent Scope Guard

## Summary

Prevent ACP permission history and pending permission modal data from showing
records that belong to a different active agent when users switch agents.

## Background

The frontend stored permission state in shared in-memory lists. During active
agent switches, stale records could remain visible briefly until the next poll
completed, causing permission history to appear mixed across agents.

## Scope

- `web/src/app.tsx`
- `web/src/app.permission_scope.test.ts`
- `docs/todo.md`

## Key Decisions

1. Scope permission pending/history rendering by `activeAgent` in frontend.
2. Clear permission pending/history state immediately on `activeAgent` switch
   to avoid stale cross-agent residue.
3. Add stale async response guard so permission poll updates are ignored when
   the active agent changed during in-flight requests.

## Validation

```bash
cd web
npm run test -- src/app.permission_scope.test.ts
```

Expected outcomes:

- Scope helper tests pass.
- Permission modal/history no longer shows records from previously selected
  agents.
- Pending permission count map remains stable when global polling excludes the
  currently active agent during switches.

## Follow-ups

- Consider adding a Playwright test that validates scoped permission indicators
  across rapid team/agent switching in a real browser session.
