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

## Additional Follow-up

After the next CI pass surfaced a stale `Web` failure and a few remaining low-risk review notes, I applied a second small cleanup pass:

- updated `web/src/acp_conversation_render.test.tsx` expectations to match the current ANSI span parser behavior, which now strips unsupported non-style span attributes while preserving text instead of rendering escaped tags
- set `type="button"` on `web/src/components/workbench_header_menu.tsx` to avoid implicit submit behavior if it is ever rendered inside a form
- precompiled the `<name>` / `<path>` regexes in `web/src/thread_markdown.ts` so skill block normalization no longer allocates a new `RegExp` per field lookup
- simplified the `Open thread` CTA in `web/src/pages/team_tasks_panel.tsx` so the click path is only constructed inside the already-guarded `selectedTask` branch

Validation for this follow-up pass:

```bash
cd web && npm run test -- src/acp_conversation_render.test.tsx src/workbench_header_menu.test.tsx src/pages/team_panels.test.tsx src/components/thread_rich_text.test.tsx
cd web && npm run lint -- --ignore-pattern dist-debug --ignore-pattern dist-debug-current
cd web && npm run build
```
