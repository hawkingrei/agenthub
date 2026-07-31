## Summary

- aligned Team shared-channel conversation browsing with the ACP conversation tail-window behavior;
- reused the same shared tail-window size for both views instead of keeping a smaller Team-only window;
- relaxed Team channel stick-to-bottom handling to match ACP's default near-bottom threshold.
- converted Team channel layout to the same split-shell pattern as ACP so the composer stays pinned to the bottom while the message body owns scrolling.

## Supersession

Stable conversation layout and composer-pinning rules from this note now live in
`docs/features/frontend-design.md#41-conversation-and-composer-visual-contract` and
`docs/features/team-channels-threads.md#11-composer-send-and-visibility-contract`. This journal
remains the rollout evidence for the channel/conversation alignment pass.

## Details

- `web/src/conversation.ts`
  - exported `DEFAULT_CONVERSATION_TAIL_WINDOW_SIZE` so Team channel and ACP conversation share the same tail-window boundary.
- `web/src/hooks/use_acp_conversation.ts`
  - replaced the ACP-local hardcoded tail-window value with the shared default constant.
- `web/src/pages/team_task_panel.tsx`
  - switched Team channel to the shared tail-window size;
  - removed the Team-only `24px` stick-to-bottom threshold override so channel scrolling now uses the same default threshold as ACP conversation.
- `web/src/pages/team_conversation_panel.tsx`
  - made the Team conversation wrapper a `flex min-h-0 flex-1` shell so the shared channel can fully occupy the workbench column.
- `web/src/pages/team_task_panel.tsx`
  - converted the panel root into a `flex` column with a dedicated `flex-1` scrolling body and a `shrink-0` composer footer;
  - kept the channel textarea outside the scrolling region so it remains at the bottom of the panel, matching ACP input-dock behavior;
  - fixed the body shell to be a `flex-col` container so the inner scroll region keeps its own height instead of expanding with content.
- `web/src/pages/team/use_team_conversation_actions.ts`
  - removed the old `60 -> 200` background hydration path for shared-thread conversation refresh;
  - now loads only the latest `20` conversation messages and latest `20` mailbox records per refresh so the frontend state and markdown render cost stay bounded;
  - intentionally keeps shared-thread browsing as a recent-20 tail view instead of adding in-panel history pagination.
- `web/src/pages/team/use_team_conversation_effects.ts`
  - decoupled the shared-thread SSE subscription from the active `# all` tab so Team keeps a live shared-thread stream target whenever a team with a shared conversation is selected;
  - kept the 4s fallback polling limited to the `# all` tab so only the realtime stream remains active on agent/task views.
- `web/src/pages/team/use_team_conversation_actions.ts`
  - generalized Team conversation refresh to target the currently selected Team task conversation instead of assuming the shared thread;
  - keeps the selected task id as the canonical Team SSE / message-refresh scope key so `/tasks/:task_id/messages` and `/sse/teams/:team_id/tasks/:task_id/messages` stay aligned;
  - clears message and mailbox state immediately when the selected conversation scope changes so switching from `# all` to a task thread does not briefly render stale shared-channel content under the new thread title.
- `web/src/pages/team_page.tsx`
  - updated the Team workbench connection badge to treat the selected Team conversation SSE stream as a team-wide target instead of only a conversation-tab target;
  - split shared-channel and task-thread workspace title/description handling so task threads no longer inherit `# all` chrome or channel-specific copy.
- `web/src/pages/team_task_panel.tsx`
  - split composer placeholder, refresh label, and empty-state copy between shared-channel and task-thread modes so task threads read as direct task conversations instead of `# all`.
- `web/src/pages/team_member_acp_panel.tsx`
  - converted the ACP shell wrapper above the Team member input dock into a `flex-col` container so the ACP panel stays constrained to the workbench height instead of expanding with conversation content;
  - replaced the fixed `104px` Team member ACP bottom clearance with measured input-dock height so the latest ACP bubble always stays above the dock.
  - removed the Team-only `Refresh thread` / `Load Older` controls so Team member ACP now follows the same SSE-driven surface contract as the main Agents ACP view.
- `web/src/components/input_dock.tsx`
  - added runtime height reporting and a stable `data-acp-input-dock` marker so ACP surfaces can reserve space based on the real dock height.
- `web/src/components/acp_conversation.tsx`
  - kept ACP conversation dock clearance on `scrollPaddingBottom` only, avoiding artificial visible whitespace while still preserving bottom scroll targeting above sticky input docks.
- `web/src/pages/team_panels.test.tsx`
  - updated channel conversation tests to cover the `200`-item tail window;
  - added a regression check that a small upward scroll near the bottom does not immediately exit stick-to-bottom mode.
  - added a layout regression test that locks the dedicated body-shell/composer-shell structure in place.
- `web/src/pages/team_member_acp_panel.test.tsx`
  - added a regression check that a measured Team member ACP input-dock height expands the conversation bottom padding above the dock.
- `web/src/pages/team/use_team_conversation_actions.test.tsx`
  - updated the shared-thread refresh test to lock the new `20`-item bounded-load behavior in place.
- `web/src/pages/team/use_team_conversation_effects.test.tsx`
  - added a regression check that the shared-thread SSE stream stays active outside the `# all` tab while polling still remains disabled there.
- `web/src/pages/team/use_team_conversation_actions.test.tsx`
  - added a regression check that switching conversation scope clears stale shared-thread messages before the next task-thread payload arrives.

## Validation

- `cd web && npm run test -- src/pages/team/use_team_conversation_actions.test.tsx src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm run test -- src/pages/team_member_acp_panel.test.tsx src/acp_panel.test.tsx`
- `cd web && npm run lint`
- `cd web && npm run build`
