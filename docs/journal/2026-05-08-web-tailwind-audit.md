# Web Tailwind ClassName Audit

## Date

2026-05-08

## Summary

Total `className="..."` occurrences across `web/src/`: ~269 across all .tsx/.ts files.

Files are categorized below by migration priority. The goal is to move Tailwind strings into
`web/src/ui/tailwind_classes.ts` constants or `web/src/ui/primitives.tsx` component wrappers.

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
| `web/src/pages/team_task_panel.tsx` | ~40+ | Heavy inline Tailwind. Largest single offender. Uses `rounded-xl`, `border`, `px-`, `py-`, `mt-`, `flex`, `gap-`, etc. |
| `web/src/components/agent_nodes_workbench.tsx` | ~30+ | Many inline grid/flex layout classes. Repeats `rounded-xl border border-ui-border/80 bg-white/72 px-4 py-4` pattern multiple times. |
| `web/src/pages/team_setup_panel.tsx` | ~20+ | Team setup wizard with many inline layout classes. |
| `web/src/pages/team_page_shell.tsx` | ~15+ | Team shell with repeated `flex`, `gap`, `px` patterns. |
| `web/src/pages/team_mailbox_panel.tsx` | ~15+ | Mailbox panel with forms/layouts. |
| `web/src/pages/team_conversation_panel.tsx` | ~10+ | Conversation panel with bubble/thread classes. |
| `web/src/app.tsx` | ~4 | Low count but high impact. Uses `text-slate-*` instead of notion tokens. |
| `web/src/components/agents_root_page.tsx` | ~8 | Root page layout with bg-white, flex, overflow-hidden. |
| `web/src/components/workspace_panel_loading_fallback.tsx` | ~2 | Small but should use tokens. |
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
| `web/src/pages/team_setup_panel.tsx` | ~20 | Setup wizard. |
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

1. **`rounded-xl border border-ui-border/80 bg-white/72 px-4 py-4`** — appears in at least 4 files
   (`agent_nodes_workbench`, `team_task_panel`, `team_setup_panel`). Should become a
   `SECTION_CARD_CLASS` constant.

2. **`flex h-full min-h-0 flex-col overflow-auto`** — workspace panel root pattern, repeated in
   ~6 files. Should become a `WORKSPACE_PANEL_ROOT_CLASS`.

3. **`uppercase tracking-[0.08em]`** — section header pattern on `<Text>` components.
   Should become a `SECTION_HEADER_CLASS`.

4. **`text-slate-900` / `text-slate-600`** — appears in `app.tsx`. Should use `text-notion-text` tokens.

5. **`px-4 py-2 sm:px-6`** — content padding pattern, repeated in ~4 files.

## Recommended Migration Order

1. Extract the 5 repeated anti-patterns above into `tailwind_classes.ts` first. (~1h)
2. Migrate `agent_nodes_workbench.tsx` — highest density of inlinable patterns. (~2h)
3. Migrate `team_task_panel.tsx` — largest file, most classes. (~3h)
4. Migrate `team_setup_panel.tsx` and `team_page_shell.tsx`. (~2h)
5. Clean up `app.tsx` slate-* colors. (~0.5h)
6. Remaining Tier 2 files. (~3h)

Total estimated effort: ~12h for complete migration.
