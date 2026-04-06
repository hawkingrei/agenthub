# Team Route Review Follow-ups

## Why

PR follow-up review on the Team/ACP frontend still had a few low-risk correctness and maintainability gaps:

- the mobile Team-page E2E helper raced the workspace overflow menu and missed `Runs`
- `AcpDebugSlot` cached a rejected lazy-import promise forever
- `TeamMemberAcpPanel` still exposed a dead `onRefresh` prop after the Team-only refresh UI was removed
- permission-poll agent id parsing did not trim whitespace
- Team thread markdown used `dangerouslySetInnerHTML`, so the sanitization contract needed an explicit regression test

## What Changed

- Hardened the mobile Team-page Playwright helper in `web/tests/e2e/team_page.e2e.ts`:
  - try an immediate visible-count check after opening `Open more workspace actions`
  - fall back to one short `150 ms` tick instead of a fixed `750 ms` wait
  - close the menu if the item is still absent so later fallbacks are not blocked by a stale menu surface
- Hardened `web/src/components/acp_panel.tsx` lazy debug loading:
  - reset the cached module promise when the import rejects
  - allow a later Debug-tab visit to retry instead of staying permanently poisoned until reload
- Removed the dead Team ACP `onRefresh` wiring so `TeamMemberAcpPanel` no longer receives an unused refresh prop from `web/src/pages/team_page.tsx`.
- Trimmed agent ids in `web/src/app_permission_polling.ts` before building global permission-poll batches.
- Added focused Team markdown regressions in `web/src/pages/team/team_markdown.test.ts` and `web/src/pages/team/team_thread_rich_text.test.tsx` proving raw HTML stays escaped and unsafe `javascript:` links do not survive rendering.
- Centralized Team conversation mailbox reset paths in `web/src/pages/team/use_team_conversation_actions.ts` so scope changes, empty thread states, and refresh failures share one clearing path.
- Centralized Team member ACP SSE teardown in `web/src/pages/team/use_team_member_acp_effects.ts` so reconnect, error, and unmount cleanup share one helper.
- Tightened the Team thread rich-text boundary in `web/src/pages/team/team_thread_rich_text.tsx`:
  - renamed the HTML override contract to `renderSanitizedHtml`
  - kept the override limited to pre-sanitized renderers such as mention-aware markdown expansion
- Reused the shared IME helper by having `web/src/pages/team/team_text_helpers.ts` delegate to `web/src/input_ime.ts` instead of maintaining a second copy of the composition guard.
- Added focused utility coverage in:
  - `web/src/app_live_output.test.ts`
  - `web/src/app_viewport.test.ts`
  so the extracted live-output routing and viewport sync helpers now have direct, file-local regression tests instead of relying only on the larger `app.permission_scope.test.ts`.

## Validation

Commands run locally:

```bash
cd web && npm run test -- vite.config.test.ts src/app.permission_scope.test.ts src/pages/team/team_markdown.test.ts src/pages/team_member_acp_panel.test.tsx src/pages/team_panels.test.tsx
cd web && npm run test -- src/app_live_output.test.ts src/app_viewport.test.ts src/pages/team/team_thread_rich_text.test.tsx src/pages/team_member_acp_panel.test.tsx src/pages/team_panels.test.tsx
cd web && npm run test -- tests/e2e/team_page.e2e.ts --grep "team page keeps single-column proportions on mobile viewport"
cd web && npm run lint -- --ignore-pattern dist-debug --ignore-pattern dist-debug-current
cd web && npm run build -- --sourcemap
make build-web
```

Notes:

- The local Playwright command is still sensitive to web-server startup timing in this environment, so the authoritative regression guard here is the updated helper plus CI.
- The Team markdown regression is explicit now, so the `dangerouslySetInnerHTML` path no longer relies on an implied sanitization contract.
