# Team Workspace Context Rerender Split

## Summary

Team workspace shell consumers no longer subscribe to the full workbench runtime context. Conversation-only updates can now leave shell-context consumers cold when their own shell fields do not change.

## Background

The frontend performance hardening TODO moved from row-local list guards to cross-page rerender audits. The Team workspace provider already exposed shell, conversation, and task context slices, but the shell slice still carried the full `workbench` object. Because `workbench` contains conversation, task, ACP, events, debug, and mailbox sub-contexts, any sub-context reference change could broadcast through the shell context and wake shell consumers.

## Scope

- Added a dedicated workbench runtime context in `web/src/pages/team/team_workspace_context.tsx`.
- Removed `workbench` from `TeamWorkspaceShellContextValue` so shell consumers subscribe only to shell fields.
- Updated `TeamWorkbenchContainer` to read the dedicated runtime context plus the shell slice it actually needs.
- Stabilized `TeamConversationContainer`'s `onJumpToMessageSettled` callback so focus-reset behavior does not create a fresh prop on every render.
- Added a regression test proving a conversation-only `taskMessageDraft` update does not rerender a memoized shell consumer, while shell and runtime changes still rerender their own consumers.

## Key Decisions

- Keep `useTeamWorkspace()` compatible for existing broad consumers and tests, but route workbench runtime ownership through `useTeamWorkbenchRuntime()` for the container that needs it.
- Do not flatten or re-own the workbench runtime object in this slice. The goal is to prevent unrelated shell consumers from subscribing to it, not to rewrite the whole Team workbench assembly.
- Keep the broader performance TODO open because explicit extremely long history behavior and any follow-up browser/profiler audit are still separate evidence.

## Validation

```bash
cd web && npm exec vitest -- run src/pages/team_panels.test.tsx
cd web && npm run build
git diff --check
```

## Follow-Ups

- Validate extremely long history behavior explicitly before closing the frontend performance TODO.
- Add browser/profiler evidence if future cross-page rerender work claims broad page-level completion.
