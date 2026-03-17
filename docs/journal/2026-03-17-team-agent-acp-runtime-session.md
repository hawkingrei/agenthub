# 2026-03-17 Team Agent ACP Runtime Session Fallback

## Summary

- `Teams -> Agent ACP` no longer requires a Team active run before the ACP panel can render.
- When the selected Team member has no active run snapshot but still has a runtime session in Team
  runtime state, the ACP panel now uses that runtime `session_id` to load and render agent events.
- `Teams -> Agent ACP` now includes a direct ACP input dock so the selected Team agent can be
  prompted from the Team workspace instead of forcing users back to the standalone Agents page.

## Why

The previous Team implementation mixed two different scopes:

- Team execution views that are correctly tied to the selected active run;
- Agent ACP inspection, which should be tied to the selected agent session.

That caused a bad failure mode on the live Team page:

- the `Agent ACP` menu entry was visible;
- clicking it without an active Team run always fell back to `No Active Run`;
- the ACP panel could not render anything even when the selected agent was already running and had a
  live runtime session.

## What Changed

- `web/src/pages/team/state.ts`
  - marked `agent_acp` as a tab that does not require a Team active run.
- `web/src/pages/team_page.tsx`
  - derived a Team-member ACP session from `snapshot.latest_step.runtime_handle_id` first, then
    from Team runtime `members[].session_id` as a fallback;
  - removed the render-time `activeRunForSelectedTeam` gate from `Agent ACP`;
  - added automatic member-event loading when opening `Agent ACP` or `Member Console` with a valid
    agent session, plus lightweight polling while those views stay open;
  - added direct ACP prompt submission using the same `api.sendInput(...)` path as the standalone
    Agents page, including session-mismatch retry against the runtime session id.
- `web/src/pages/team/use_team_actions.ts`
  - updated `loadMemberEvents()` to support explicit `selectedMemberId` and `selectedMemberSessionId`
    fallback even when no Team run snapshot exists.
- `web/src/pages/team/use_team_mailbox_lifecycle_effects.ts`
  - limited mailbox-only member selection normalization to the `mailbox` tab so opening
    `Agent ACP` or `Member Console` without a Team run snapshot no longer clears the selected Team
    member out from under the ACP view.
- `web/src/pages/team_member_acp_panel.tsx`
  - allowed the ACP panel to render from an explicit session id / role fallback instead of requiring
    a `selectedMemberSnapshot`;
  - added an ACP input dock with local command history and direct send support.
- Tests
  - added hook coverage for loading agent events from runtime session fallback;
  - updated mailbox lifecycle coverage so missing Team snapshots only reset member selection inside
    the `mailbox` tab and preserve agent selection for `Agent ACP`;
  - added panel coverage for rendering ACP conversation without a Team run snapshot and for sending
    prompts through the Team ACP input dock.

## Validation

- `cd web && npx vitest run src/pages/team_panels.test.tsx src/pages/team/use_team_actions.test.tsx`
- `cd web && npm run lint -- src/pages/team_page.tsx src/pages/team_member_acp_panel.tsx src/pages/team/use_team_actions.ts src/pages/team/state.ts src/pages/team_panels.test.tsx src/pages/team/use_team_actions.test.tsx`
- `cd web && npm run build`

## Chrome MCP

- Live baseline on `https://agenthub.hawkingrei.com/teams/<team_id>` showed:
  - `Agent ACP` was available in the advanced menu;
  - after the first runtime-session fallback fix, clicking it started loading agent events but still
    rendered `Select an agent from the left rail to inspect its thread.`;
  - workspace details showed `member=-` while the agent workspace header still showed the focused
    agent, proving mailbox lifecycle effects were clearing the selected member during ACP tab
    switches without an active Team snapshot.
- Post-edit live regression remains blocked until this change is deployed because verification must
  stay on the production domain.
