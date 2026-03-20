# 2026-03-17 Team Agent Workspace Focus

## Summary

- Clicking an agent in the Team sidebar now keeps the main workspace agent-first instead of falling
  back to a Team-level mailbox view.
- Agent-focused workspace state is now tracked separately from the generic mailbox member
  selection.

## Why

The previous Team UI overloaded one piece of state (`selectedMemberId`) for two different jobs:

- the selected recipient/conversation inside the Team mailbox;
- the selected agent workspace from the left sidebar.

That made the main workspace ambiguous:

- clicking an agent could still render a generic `Mailbox` view;
- the header continued to show Team goal/runtime controls that belonged to the Team workspace, not
  the selected agent;
- team mailbox auto-selection could accidentally look like an agent workspace.

## What Changed

- `web/src/pages/team_page.tsx`
  - added a dedicated `focusedAgentMemberId` state for true left-rail agent focus;
  - agent focus is now cleared when switching back to Team-level `all`, `Kanban`, `Runs`, or Team
    utility views;
  - clicking a Team agent now defaults to `Agent ACP` instead of opening `Execution Mailbox`;
  - `Execution Mailbox` stays available from `Advanced`, so run-scoped mailbox/debug views no
    longer displace the agent's primary conversation surface;
  - agent workspace header now shows agent-first metadata (`Role`, `Lifecycle`, `Work`, `Inbox`,
    `Current Work`) instead of Team goal/runtime controls;
  - agent-focused mailbox empty state now stays concise and action-oriented instead of repeating the
    same agent status strip already shown in the workspace header.
- `web/src/pages/team_sidebar.tsx`
  - sidebar agent highlight now follows `focusedAgentMemberId` instead of mailbox recipient
    selection;
  - clicking an agent in the left rail now routes to `agent_acp`.
- `web/src/pages/team/page_helpers.ts`
  - added pure helpers for focused-agent label resolution and agent workspace status summarization.
- `web/src/pages/team/page_helpers.test.ts`
  - added focused helper coverage for agent label fallback and status summary rendering.
- `web/src/pages/team_member_acp_panel.tsx`
  - switched Team Agent ACP onto the shared `useAcpConversation` hook so old-message loading,
    frozen-scroll behavior, jump-to-bottom, and `agent_thought` rendering align with the standalone
    Agents conversation surface.
- `web/src/pages/team_panels.test.tsx`
  - added regressions for Team Agent ACP auto-loading older history on short threads and surfacing
    `agent_thought` content inside the Team workspace.

## Validation

- `cd web && npx vitest run src/pages/team_page.runs.test.ts src/pages/team/page_helpers.test.ts`
- `cd web && npx vitest run src/pages/team_panels.test.tsx src/pages/team/page_helpers.test.ts src/pages/team/use_team_conversation_effects.test.tsx`
- `cd web && npm run lint -- src/pages/team/page_helpers.ts src/pages/team/page_helpers.test.ts src/pages/team_sidebar.tsx src/pages/team_panels.test.tsx src/pages/team_page.tsx`
- `cd web && npm run build`

## Chrome MCP

- Live baseline on `https://agenthub.hawkingrei.com/teams/<team_id>` showed that clicking a left
  rail agent still landed in a Team-level mailbox surface with Team goal/runtime controls.
- Post-edit live regression remains blocked until this change is deployed because verification must
  stay on the production domain.
