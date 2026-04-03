## Summary

- aligned Team shared-channel conversation browsing with the ACP conversation tail-window behavior;
- reused the same shared tail-window size for both views instead of keeping a smaller Team-only window;
- relaxed Team channel stick-to-bottom handling to match ACP's default near-bottom threshold.

## Details

- `web/src/conversation.ts`
  - exported `DEFAULT_CONVERSATION_TAIL_WINDOW_SIZE` so Team channel and ACP conversation share the same tail-window boundary.
- `web/src/hooks/use_acp_conversation.ts`
  - replaced the ACP-local hardcoded tail-window value with the shared default constant.
- `web/src/pages/team_task_panel.tsx`
  - switched Team channel to the shared tail-window size;
  - removed the Team-only `24px` stick-to-bottom threshold override so channel scrolling now uses the same default threshold as ACP conversation.
- `web/src/pages/team_panels.test.tsx`
  - updated channel conversation tests to cover the `200`-item tail window;
  - added a regression check that a small upward scroll near the bottom does not immediately exit stick-to-bottom mode.

## Validation

- `cd web && npm run test -- src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm run lint`
- `cd web && npm run build`
