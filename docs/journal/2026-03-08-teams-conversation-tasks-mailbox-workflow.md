# Teams Conversation/Tasks/Mailbox Workflow

## Summary

This change turns the Team workspace into three first-class user-facing tabs:

- `Conversation`
- `Tasks`
- `Mailbox`

The goal is to keep the Slock-inspired subject-first layout while making Team task planning and member-directed mailbox chat explicit instead of burying them behind debug-only controls.

## Key Decisions

- Keep the shared thread as a single public `all` conversation.
- Keep Team task objects as the execution/planning model instead of introducing a new backend thread/channel schema in this change.
- Add a dedicated `Tasks` workspace that surfaces task creation, compile preview, and run creation.
- Keep `Mailbox` as a first-class Team tab instead of an advanced-only tool.
- Treat `seen by N agents` as mailbox delivered/ack coverage for the matching shared-thread message, not as strict browser read receipts.
- Prefer human-readable agent names in the Team UI and mention rendering, while preserving canonical `member_id` routing under the hood.

## Implementation Notes

- Added a first-class `Tasks` panel in `web/src/pages/team_tasks_panel.tsx`.
  - task list with status filters
  - selected task detail
  - compile preview
  - create run from compiled preview
- Promoted Team-level primary tabs to `Conversation`, `Tasks`, and `Mailbox`.
- `Conversation`
  - keeps the shared `all` thread
  - merges human-visible mailbox chat replies
  - renders `seen by N agents`
  - expands into the specific delivered/acked agent names on demand
- `Mailbox`
  - stays team-level primary UI
  - member-directed mailbox chat remains available from the main Team surface
- Left-rail agent selection now defaults to member-scoped `Mailbox` instead of jumping directly into ACP-only views.
- Agent-focused `Mailbox` still exposes `Agent ACP`, `Member Console`, and `Debug` through `Advanced`, so ACP inspection remains available without competing with the primary Team tabs.
- Mention rendering now prefers display names:
  - visible UI uses `@agent_name`
  - send path canonicalizes back to the stable actor identity used by mailbox routing

## Validation

- `cd web && npm run lint`
- `cd web && npx vitest run src/pages/team/page_helpers.test.ts src/pages/team_panels.test.tsx --pool=threads --maxWorkers=1`
- `make build-web`
- `cd web && PLAYWRIGHT_NO_WEBSERVER=1 npm run e2e -- tests/e2e/team_page.e2e.ts`

## Chrome MCP Notes

- Baseline MCP checks were taken on:
  - `https://app.slock.ai/channel/dfed28bb-5d1c-4ee2-99fe-5ed773e1fa09`
  - `https://agenthub.hawkingrei.com/teams`
- The authenticated Slock view confirmed the target shape:
  - left rail as subject index
  - `all` as the shared thread
  - tasks as a first-class workspace
  - message rows defaulting to `author + time + body`
- Live deployed AgentHub still reflects the previously deployed bundle, so the after-edit product regression for these new local changes cannot be confirmed on the domain until deployment.
- After-edit Chrome MCP was run against the local Vite dev server (`http://127.0.0.1:5173`), but without a matching local backend session it redirects to the login shell only. Product-level after validation for this change therefore relies on the full Playwright file plus local web build/test until the branch is deployed.

## Follow-Up

- Add server-backed per-agent read receipts if strict `read by` semantics are required later.
- Consider exposing a richer Team task metadata model if Tasks should become more than a compile/run staging surface.
