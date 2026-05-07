# Web Tailwind ClassName Audit

## Summary

This checkpoint records the current Tailwind/className migration state across `web/src/`.

- total `className="..."` occurrences across `web/src/`: about `~269`
- current migration goal: move repeated Tailwind strings into
  `web/src/ui/tailwind_classes.ts` constants or `web/src/ui/primitives.tsx`
  wrappers
- current remaining estimate after the latest Team setup/mailbox constant pass:
  about `~3h`

## Background

The repo already moved part of the Team/workspace chrome onto shared primitives,
but repeated inline Tailwind strings still drift across Team-heavy panels and
workspace shells. This audit exists to keep the next cleanup passes pointed at
the largest remaining offenders instead of letting ad-hoc class extraction
happen file by file.

## Scope

This audit covers `web/src/` TS/TSX source files only.

It is meant to capture:

- migration priority by file
- repeated Tailwind anti-patterns worth extracting
- the recommended next migration order

It is not a canonical frontend design spec and does not replace
`docs/features/frontend-design.md`.

## Key Decisions

- treat repeated section-shell and header-shell literals as the first
  extraction target
- record the post-change state, not the pre-migration snapshot
- keep `auth_pages.tsx` and the auth gate copy marked as resolved for the
  `text-slate-*` cleanup
- leave partially migrated files explicitly marked as partial instead of
  overstating them as complete

## Audit by File

### Tier 0 — Already Using Primitives / Constants Only

- `web/src/ui/primitives.test.tsx` — test file, all className calls exercise primitives. ✅ OK.
- `web/src/ui/tailwind_classes.ts` — defines constants (no JSX). ✅ OK.
- `web/src/ui/primitives.tsx` — defines BASE_CLASS constants. ✅ OK.
- `web/src/ui/floating_surfaces.ts` — defines constants. ✅ OK.
- `web/src/ui/mantine_theme.ts` — config, not JSX. ✅ OK.

### Tier 1 — High Priority (Production Code With Inline Tailwind)

| File | Approx Count | Notes |
|---|---|---|
| `web/src/pages/team_task_panel.tsx` | ~reduced | **Partially resolved:** the 2026-04-11 primitive pass moved empty states, dense metadata controls, compact action rows, badges, and conversation bubbles onto shared primitives. Remaining inline classes are mostly message/activity layout and advanced permission-card variants. |
| `web/src/components/agent_nodes_workbench.tsx` | ~30+ | Many inline grid/flex layout classes. **Partially resolved:** the repeated section-card literal is now `SECTION_CARD_CLASS`, and section headers use `SECTION_HEADER_CLASS`. Still has other inline patterns to extract. |
| `web/src/pages/team_setup_panel.tsx` | ~reduced | **Partially resolved:** setup action grid, info-strip grid/step shell, setup checklist, and copy-action button now use shared Tailwind constants. Remaining inline classes are local title/content structure. |
| `web/src/pages/team/team_page_shell.tsx` | ~reduced | **Partially resolved:** shell root now uses a shared Team shell constant. Remaining layout comes from route-owned pane composition. |
| `web/src/pages/team_mailbox_panel.tsx` | ~reduced | **Partially resolved:** mailbox meta rows, member row chrome, chat header/status, message body/fallback, composer, and advanced mailbox form chrome now use shared Tailwind constants. Remaining inline classes are mostly local alignment and semantic hooks. |
| `web/src/pages/team_conversation_panel.tsx` | ~10+ | Conversation panel with bubble/thread classes. |
| `web/src/app.tsx` | ~reduced | **Partially resolved:** AuthGateCard uses notion text tokens, and route app roots now use `APP_ROOT_CLASS`. Remaining inline classes are auth copy typography. |
| `web/src/pages/auth_pages.tsx` | ~0 | ✅ RESOLVED. Previously used `text-slate-*`; now uses notion tokens. |
| `web/src/components/agents_root_page.tsx` | ~reduced | **Partially resolved:** shared workspace content root and error-banner padding constants now cover the repeated route shell pieces. |
| `web/src/components/workspace_panel_loading_fallback.tsx` | ~reduced | Uses the shared loading-shell constant while keeping local title/body structure. |
| `web/src/components/create_agent_modal.tsx` | ~1 | `min-w-0` only. |

### Tier 2 — Medium Priority

| File | Approx Count | Notes |
|---|---|---|
| `web/src/pages/team_active_run_panel.tsx` | ~5 | Run panel layout. |
| `web/src/pages/team_steps_panel.tsx` | ~4 | Steps panel. |
| `web/src/pages/team_member_acp_panel.tsx` | ~5 | Member ACP. |
| `web/src/pages/team_member_status_strip.tsx` | ~5 | Status strip. |
| `web/src/pages/team_thread_pane.tsx` | ~8 | Thread pane layout. |
| `web/src/pages/team_sidebar.tsx` | ~5 | Sidebar. |
| `web/src/pages/team_run_panel.tsx` | ~5 | Run panel. |
| `web/src/pages/team_management_modals.tsx` | ~5 | Management modals. |
| `web/src/pages/workspace_page.tsx` | ~3 | Workspace root. |
| `web/src/pages/agent_page.tsx` | ~5 | Agent detail. |
| `web/src/pages/agent_nodes_page.tsx` | ~3 | Node list. |
| `web/src/routes/route_fallback.tsx` | ~2 | Fallback page. |
| `web/src/routes/admin_route_container.tsx` | ~1 | Admin container. |
| `web/src/routes/agents_route_container.tsx` | ~2 | Agent container. |

### Tier 3 — Low / Special Cases

| File | Approx Count | Notes |
|---|---|---|
| `web/src/components/acp_panel.tsx` | ~5 | ACP rendering, mostly uses ACP_* constants already. |
| `web/src/components/acp_debug_panel.tsx` | ~5 | Debug panel. |
| `web/src/components/agent_list_panel.tsx` | ~3 | Agent list. |
| `web/src/components/agent_output_panel.tsx` | ~3 | Output display. |
| `web/src/rich_text_classes.ts` | — | Constants file (defines, not uses). |
| Various `.test.tsx` files | ~5 | Test-specific className values (not product code). |

## Repeated Anti-Patterns

1. **`rounded-xl border border-ui-border/80 bg-white/72 px-4 py-4`** — ✅ RESOLVED.
   Extracted as `SECTION_CARD_CLASS` in `tailwind_classes.ts`, migrated in `agent_nodes_workbench.tsx`.

2. **`flex h-full min-h-0 flex-col overflow-auto`** — ✅ PARTIALLY RESOLVED.
   Extracted as `WORKSPACE_PANEL_ROOT_CLASS` and migrated in the node workbench.

3. **`uppercase tracking-[0.08em]`** — ✅ RESOLVED.
   Extracted as `SECTION_HEADER_CLASS` in `tailwind_classes.ts`, migrated in `agent_nodes_workbench.tsx`
   and `agent_node_detail_shared.tsx`.

4. **`text-slate-900` / `text-slate-600`** — ✅ RESOLVED.
   Fixed in both `app.tsx` AuthGateCard and `pages/auth_pages.tsx`.

5. **`px-4 py-2 sm:px-6`** — ✅ PARTIALLY RESOLVED.
   Extracted as `WORKSPACE_CONTENT_PADDING_CLASS` for the agents route error banner.

## Recommended Migration Order

1. ✅ Extract the highest-value repeated section primitives into `tailwind_classes.ts`.
   - `SECTION_CARD_CLASS`
   - `SECTION_HEADER_CLASS`
2. ✅ Migrate `agent_nodes_workbench.tsx` to the new shared section primitives.
3. ✅ Clean up the remaining `text-slate-*` auth copy in `app.tsx` and `auth_pages.tsx`.
4. ✅ Migrate the first `team_task_panel.tsx` primitive wave.
5. ✅ Extract the next repeated layout/padding patterns:
   - `WORKSPACE_PANEL_ROOT_CLASS`
   - `WORKSPACE_CONTENT_ROOT_CLASS`
   - `WORKSPACE_CONTENT_PADDING_CLASS`
   - Team setup info-strip/action grid constants
6. ✅ Continue `team_setup_panel.tsx` and `team_mailbox_panel.tsx` constant migration.
7. Continue Team shell/detail surfaces and the remaining Tier 2 files. (~3h)

## Validation

- reviewed current `web/src/` className surfaces after the latest primitive and
  token extraction pass
- updated the audit to reflect the post-change state of:
  - `agent_nodes_workbench.tsx`
  - `agent_node_detail_shared.tsx`
  - `app.tsx`
  - `auth_pages.tsx`
  - `agents_root_page.tsx`
  - `workspace_panel_loading_fallback.tsx`
  - `team_setup_panel.tsx`
  - `team/team_page_shell.tsx`
  - `team_conversation_panel.tsx`
  - `team_mailbox_panel.tsx`
- aligned the remaining-effort wording with `docs/todo.md`

## Follow-Ups

- continue Team shell/detail and Tier 2 panel migrations where inline layout
  strings still repeat
- migrate remaining route shell roots and loading/error wrappers as they repeat
- keep `docs/todo.md` pointing at the current remaining estimate instead of
  duplicating stale audit prose

Total estimated remaining effort: ~3h.
