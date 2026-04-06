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
  - wait briefly for the overflow menu item after opening `Open more workspace actions`
  - close the menu if the item is absent so later fallbacks are not blocked by a stale menu surface
- Hardened `web/src/components/acp_panel.tsx` lazy debug loading:
  - reset the cached module promise when the import rejects
  - allow a later Debug-tab visit to retry instead of staying permanently poisoned until reload
- Removed the unused `onRefresh` prop from `web/src/pages/team_member_acp_panel.tsx` and updated focused tests.
- Trimmed agent ids in `web/src/app_permission_polling.ts` before building global permission-poll batches.
- Added a focused Team markdown regression in `web/src/pages/team/team_markdown.test.ts` proving raw HTML stays escaped and unsafe `javascript:` links do not survive rendering.

## Validation

Commands run locally:

```bash
cd web && npm run test -- vite.config.test.ts src/app.permission_scope.test.ts src/pages/team/team_markdown.test.ts src/pages/team_member_acp_panel.test.tsx src/pages/team_panels.test.tsx
cd web && npm run test -- tests/e2e/team_page.e2e.ts --grep "team page keeps single-column proportions on mobile viewport"
cd web && npm run lint -- --ignore-pattern dist-debug --ignore-pattern dist-debug-current
cd web && npm run build -- --sourcemap
make build-web
```

Notes:

- The local Playwright command is still sensitive to web-server startup timing in this environment, so the authoritative regression guard here is the updated helper plus CI.
- The Team markdown regression is explicit now, so the `dangerouslySetInnerHTML` path no longer relies on an implied sanitization contract.
