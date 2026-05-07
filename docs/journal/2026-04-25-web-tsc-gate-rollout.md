# Web `tsc --noEmit` Gate Rollout

## Goal

Pay down the historical TypeScript backlog in `web/` until `npm exec tsc -- --noEmit` is clean, then wire the same check into CI so frontend type regressions fail the main `Web` workflow.

## Local cleanup result

Validated locally on the default frontend workspace:

```bash
cd web && npm exec tsc -- --noEmit
```

Result: passes with zero TypeScript errors.

## Rollout notes

- Cleared the backlog in stages instead of trying to rewrite the frontend type surface in one pass.
- Fixed the highest-leverage common typing surface first (`ui/primitives.tsx`) so wrapper prop typing stopped cascading through unrelated tests and pages.
- Then aligned historical test fixtures and mocks with the current app model:
  - `AuthState`
  - `TeamActorMessageRecord`
  - `TeamTaskRecord`
  - `TeamRunSnapshotRecord`
  - Team ACP / Team workspace draft types
- Narrowed remaining implementation errors after fixture cleanup:
  - ACP/tool payload unions
  - `unknown` / `null` narrowing in Team hooks and panels
  - `ArrayBuffer` compatibility edges in push / webauthn helpers
  - `.at()` usage on targets that are below the current TS lib floor

## CI rollout

The `Web` workflow now includes:

```yaml
- name: Typecheck
  run: npm exec tsc -- --noEmit
  working-directory: web
```

This keeps the typecheck inside the existing frontend required path instead of creating a second partially-overlapping workflow.

## Local verification

Commands run during this rollout:

```bash
cd web && npm exec tsc -- --noEmit
cd web && npm exec vitest -- run src/ui/primitives.test.tsx src/acp_panel.test.tsx src/acp_conversation_render.test.tsx src/create_agent_modal.test.tsx src/pages/admin_page.test.tsx src/pages/team_member_acp_panel.test.tsx src/pages/team_panels.test.tsx src/pages/team/use_team_management_actions.test.tsx src/pages/team/use_team_conversation_actions.test.tsx src/pages/team/use_team_task_workspace_data.test.tsx src/pages/team/use_team_workspace_view_model.test.tsx src/pages/team/use_team_catalog_view_model.test.tsx src/pages/team/use_team_run_lifecycle_effects.test.tsx src/pages/team/use_team_mailbox_actions.test.tsx src/pages/team_page.smoke.test.tsx src/use_app_agents.test.tsx src/use_app_admin.test.tsx src/output_error_boundary.test.tsx src/app_viewport.test.ts src/input_dock_keyboard.test.ts src/output_cache_storage.test.ts src/pages/team_sidebar.helpers.test.ts src/pages/team/team_workspace_header.test.tsx src/pages/team/runtime_cache_storage.test.ts src/pages/team/mailbox_helpers.test.ts src/pages/team_page.runs.test.ts src/conversation_window.test.ts src/app_live_output.test.ts
cd web && npm run build
```

Results:

- `tsc --noEmit`: pass
- targeted Vitest batch: `28` files, `371` tests passed
- `npm run build`: pass

## Closure evidence

- Pull request evidence: PR #531 `Web` check passed on run `25470904818`, job `74734378747`; the job included a successful `Typecheck` step running `npm exec tsc -- --noEmit`.
- Main push evidence: merge commit `05b5b5ff6662241733895b10c97c3123a7a0981d` passed the `Web` workflow on run `25471294329`, job `74735547028`; the job included a successful `Typecheck` step.
- Gate shape: `.github/workflows/web.yml` keeps `Typecheck` between `Lint` and `Test with coverage`, so type regressions fail the existing `Web` frontend gate instead of relying on an optional side workflow.
