## Summary

- aligned Team shared-channel conversation browsing with the ACP conversation tail-window behavior;
- reused the same shared tail-window size for both views instead of keeping a smaller Team-only window;
- relaxed Team channel stick-to-bottom handling to match ACP's default near-bottom threshold.
- converted Team channel layout to the same split-shell pattern as ACP so the composer stays pinned to the bottom while the message body owns scrolling.

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
  - now loads only the latest `20` conversation messages and latest `20` mailbox records per refresh so the frontend state and markdown render cost stay bounded.
- `web/src/pages/team_panels.test.tsx`
  - updated channel conversation tests to cover the `200`-item tail window;
  - added a regression check that a small upward scroll near the bottom does not immediately exit stick-to-bottom mode.
  - added a layout regression test that locks the dedicated body-shell/composer-shell structure in place.
- `web/src/pages/team/use_team_conversation_actions.test.tsx`
  - updated the shared-thread refresh test to lock the new `20`-item bounded-load behavior in place.

## Validation

- `cd web && npm run test -- src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm run lint`
- `cd web && npm run build`
