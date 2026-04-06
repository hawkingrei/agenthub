# Frontend Follow-up Review Fixes

## Why

After the recent Team/ACP frontend refactors, PR review still flagged a few small but real issues:

- permission polling used hard-coded reschedule delays instead of named constants
- terminal ANSI parsing assumed `<span>` tags only carried inline `style`
- Team conversation fallback polling could stay stale after tab/auto-refresh gating changes
- Team setup/workspace state panels duplicated workbench accent/badge styles

These were low-risk fixes and aligned with the maintainability policy for active changes.

## What Changed

- Introduced explicit permission poll reschedule constants in `web/src/app_permission_polling.ts`:
  - `GLOBAL_PERMISSION_POLL_ACTIVE_DELAY_MS`
  - `GLOBAL_PERMISSION_POLL_IDLE_DELAY_MS`
- Extended the ANSI span parser in `web/src/components/acp_terminal_output.tsx` so renderer-emitted `class="..."` attributes do not break terminal text extraction.
- Updated `web/src/pages/team/use_team_conversation_effects.ts` to recompute fallback polling immediately when tab/auto-refresh gating changes, instead of waiting for another unrelated state transition.
- Moved shared Team workbench accent/badge class strings into `web/src/ui/tailwind_classes.ts` and reused them from:
  - `web/src/pages/team_setup_panel.tsx`
  - `web/src/pages/team_workspace_state_panel.tsx`

## Validation

Commands run locally:

```bash
cd web && npm run test -- src/app_permission_polling.test.ts src/acp_conversation.test.ts src/pages/team/use_team_conversation_effects.test.tsx src/pages/team_panels.test.tsx
cd web && npm run lint -- --ignore-pattern dist-debug --ignore-pattern dist-debug-current
cd web && npm run build
```
