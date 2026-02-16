# Web Render Isolation Memoization

## Summary

Reduce unnecessary frontend re-renders by isolating heavy workspace panels from
high-frequency input state updates.

## Background

`web/src/app.tsx` manages both:

- high-frequency state updates (input typing, composition state), and
- large UI trees (agents list, output header, ACP conversation/debug panels).

Before this change, several heavy subtrees received inline object/callback props
from `App`, causing avoidable re-renders even when their effective data did not
change.

## Scope

- `web/src/app.tsx`
- `web/src/components/output_body.tsx`
- `web/src/components/acp_panel.tsx`
- `web/src/components/agents_panel.tsx`
- `web/src/components/output_header.tsx`
- `web/src/acp_panel.test.tsx`
- `docs/todo.md`

## Key Decisions

1. Add `React.memo` to panel-level components:
   - `OutputBody`
   - `AcpPanel`
   - `AgentsPanel`
   - `OutputHeader`
2. Stabilize expensive props in `App` using `useMemo`:
   - `acpRuntimeMetrics`
   - `acpConversationProps`
   - `acpDebugProps`
   - `acpPanelProps`
3. Stabilize panel callbacks using `useCallback`:
   - ACP actions (`onAcpSetMode`, `onAcpSetModel`, `onAcpSetConfig`,
     `onAcpCancel`, `onAcpClearSession`)
   - Agent row actions (`onStartAgent`, `onStopAgent`, `onDeleteAgent`,
     `onSetCodeMode`)
   - Workspace selection/toggle handlers (`handleSelectAgent`,
     `handleCollapseAgents`, `handleExpandAgents`, `handleToggleAgents`,
     `handleAcpTabSelect`)
4. Keep behavior unchanged:
   - no API contract changes
   - no output ordering changes
   - no ACP flow changes
5. Keep `acp_panel` callback test intact by exporting an internal render
   function (`AcpPanelView`) for direct event callback assertions.
6. Keep hook initialization order safe:
   - callbacks referenced by memoized prop objects must be initialized before
     those `useMemo` blocks run;
   - avoid TDZ (`Cannot access 'onAcpSetMode' before initialization`) causing a
     blank initial app shell.
7. Keep ACP debug runtime metrics fresh while preserving memoization:
   - include `acpConversation.conversationRenderItems` in
     `acpRuntimeMetrics` dependencies so cache hit/miss counters are refreshed
     when rendered conversation slices change.

## Validation

Recommended commands:

```bash
npm --prefix web run test -- src/output_body.test.tsx src/acp_panel.test.tsx src/output_header.test.tsx
npm --prefix web run e2e -- tests/e2e/app.e2e.ts
```

Manual checks:

1. Open an active ACP session and type continuously in the input dock.
2. Confirm conversation/debug panes do not flicker and interaction remains
   smooth.
3. Switch tabs (`Conversation`/`Debug`) and verify behavior is unchanged.
4. Select/collapse/expand agents and verify panel behavior is unchanged.
