# Native Recovery Controls

## Summary

Standalone and Team member threads now share an explicit recovery review panel.
Operators query the current session, inspect whether interrupted work needs review,
and submit a bounded note for the exact current recovery target. Saving never starts
a turn or replays an earlier approval.

## Background

The backend recovery endpoints had no user-facing action. A cancelled or restored
session could reject new input while leaving its operator without a way to obtain
the current recovery token and record an effect review.

## Scope

- Typed current-state query and reconciliation API calls.
- One Mantine panel shared by standalone and member workspaces.
- Explicit reads, bounded notes, exact ownership and stale-response isolation.
- Focused component, API, routing and browser coverage.

## Key Decisions

Conversation metadata identifies a native surface only. Every actionable token comes
from a current authenticated query and must match the selected local session. A query
does not consume a pending question or approval. Read-only/inactive surfaces expose
no recovery action.

Panel ownership includes authentication, actor and local launch. Late queries and
confirmation responses cannot update a replacement panel. Duplicate clicks cannot
dispatch another confirmation. A failed response clears the actionable view; the
operator refreshes before any explicit retry. There is no automatic query loop and
no input submission after confirmation.

Contract: [direct runtime integration](../features/rara-direct-integration.md).

## Validation

```bash
cd web
npm run lint
npx tsc --noEmit
npm run build
npx vitest run src/native_recovery.test.ts src/components/native_recovery_panel.test.tsx src/agents_workbench.test.tsx src/components/agents_route_shell_props.test.ts src/pages/team_member_acp_panel.test.tsx src/pages/team_page.agent_loop.test.tsx
npx playwright test tests/e2e/native_recovery.e2e.ts --project=chromium --workers=1
```

Focused checks cover UTF-8 note limits, malformed/foreign query data, exact API
identity, duplicate clicks, late responses after session replacement, uncertain
confirmation, clean sessions and existing pending questions. Browser checks use
controlled API fixtures and assert that reconciliation never calls the input endpoint.

All three frontend gates pass. The six focused test files pass 59 cases. Desktop,
narrow-screen and read-only browser cases pass; the narrow view has no horizontal
overflow and viewers have no recovery action or query. Browser evidence uses fixtures,
not an assembled backend or a configured provider.

Chrome DevTools MCP was unavailable. The repository Playwright harness captured
the existing page before integration and the recovery form afterward. Local requests
needed the loopback addresses excluded from the external HTTP proxy, and the matching
Chromium headless build was installed in a task-owned temporary cache. No application
or browser configuration file was changed for that setup.

## Follow-Ups

Qualify actual assembled process restart and restored callbacks through the browser,
reconcile ambiguous openings, finish standalone continuity and validate the installed
runtime with its configured provider. Resume preflight remains closed until that
acceptance is complete. Upstream publication still awaits its existing destination-
specific authorization.
