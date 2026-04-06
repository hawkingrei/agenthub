# Team Conversation Polling Hardening

## Summary

- reduced repeated Team task detail fetches when a selected thread still has no latest run
- stopped re-fetching thread detail on a fixed cooldown after the UI has already confirmed `latest_run = null`; the selected conversation now treats a cached `null` run id as stable instead of retrying forever
- aligned Team conversation SSE fallback so 4s polling only runs when SSE is unavailable or disconnected
- moved Team member ACP auto-sync to the same `SSE first + disconnect fallback poll` model as the Agents workbench
- tightened both Team conversation and Team member ACP fallback gating again so the 4s fallback loop stays off while `EventSource` is still in the `CONNECTING` / reconnect-attempt phase; fallback only re-enables after an explicit SSE error path
- gated root Agents workbench background SSE and permission polling so it no longer runs behind Team routes
- added a cooldown to Team member agent-card backfill so cached hidden members are not revalidated on every rerender
- pruned stale `lastResolvedAtRef` entries for Team member backfill so long-lived sessions do not retain cooldown state for members that have already left the active team spec
- kept selected task thread stable while `taskList` is temporarily stale but `selectedConversationDetail` is still present
- fixed Team thread optimistic echoes to reuse the active thread `conversation_id`
- hardened shared primitives and input handling with:
  - `SelectableListItem` defaulting to `type="button"`
  - input dock height reporter resetting when callback identity changes
  - IME-aware Enter handling in Team channel composer
  - ANSI segment cache refreshing access order on hits
- split Team markdown helpers away from the ACP wrapper path:
  - added `web/src/thread_markdown.ts`
  - added `web/src/pages/team/team_markdown.ts`
  - added `web/src/pages/team/team_thread_rich_text.tsx`
  - moved Team message rendering off the direct `components/thread_rich_text.tsx` dependency
- moved Team member ACP loading behind a route-local dynamic import in `web/src/pages/team_page.tsx`
- updated Vite chunk routing so Team-local markdown and Team member ACP have their own named boundaries (`route-rich-text-shared`, `route-teams-agent-acp`) instead of piggybacking silently on the default route graph
- introduced a dedicated `route-acp-shared` boundary for the Team member ACP shell so `/teams` no longer has to eager-load the old `route-agents-workbench` chunk just to reach ACP primitives later
- keyed Team conversation detail caches by `teamId:taskId` and clear them when the selected team changes so cooldown/run-id state cannot bleed across teams
- limited extra `getTeamTask(...)` latest-run discovery to the shared thread path; ordinary task threads now trust the selected-thread state instead of re-fetching detail on every refresh while no latest run exists
- restored a stable optimistic `conversation_id` fallback for empty threads so the client never emits an empty conversation id before the server echo lands
- delayed clearing a selected non-shared thread until task refresh has actually settled:
  - keep the selected thread while `tasksLoading` is still true
  - keep the selected thread when `selectedConversationDetail.task` still provides a fallback task record
  - only clear after the refreshed task list confirms the thread is gone
- aligned Team Tailwind theme variables with the semantic tokens already referenced by the route-level classes and tests so route-local styling no longer depends on stale theme drift between `tailwind.config.cjs` and `tailwind.css`
- extracted the duplicated LRU budgeting helpers used by ACP ANSI parsing and thread markdown rendering into a shared `web/src/cache_with_lru_budget.ts` utility so recency refresh and byte-budget eviction now stay aligned across both caches
- moved Team thread viewport/window behavior onto route-local helpers:
  - added `web/src/pages/team/team_conversation_viewport.ts`
  - stopped importing `web/src/conversation.ts` / `web/src/hooks/thread_viewport.ts` from the Team route
- moved Team HTML escaping / IME composition checks onto route-local helpers:
  - added `web/src/pages/team/team_text_helpers.ts`
  - stopped importing `web/src/html_escape.ts` / `web/src/input_ime.ts` from the Team route
- corrected Vite chunk routing again so the Team route now resolves shared markdown through `route-rich-text-shared` instead of reusing `route-agents-workbench`
- added a neutral `route-mantine-inputs` chunk for Team/ACP shared form controls
- finished the last Team-route lazy edge by moving `AcpDebug` mode/model/config controls onto lightweight native inputs so the debug-only chunk no longer owns shared Mantine form internals
- narrowed `route-mantine-inputs` matching so package-level `@mantine/core` / `@mantine/hooks` index imports stay on `vendor-mantine` while only the concrete Team/ACP input submodules pin into the shared input chunk
- added focused lazy-split regression coverage for:
  - `web/src/components/agents_route_shell.tsx`
  - `web/src/components/use_agents_workbench_panel.ts`
  - root-route `App -> AgentsRouteShell` prop wiring

## Validation

- `cd web && npm run test -- src/pages/team_panels.test.tsx src/pages/team/use_team_conversation_actions.test.tsx src/pages/team/use_team_conversation_effects.test.tsx src/pages/team/use_team_member_acp_effects.test.tsx src/ui/primitives.test.tsx`
- `cd web && npm run test -- src/pages/team/use_team_conversation_effects.test.tsx src/pages/team/use_team_member_acp_effects.test.tsx src/pages/team/use_team_member_backfill_effect.test.tsx src/cache_with_lru_budget.test.ts src/acp_conversation.test.ts src/components/thread_rich_text.test.tsx`
- `cd web && npm run test -- vite.config.test.ts src/input_dock_keyboard.test.ts src/components/thread_rich_text.test.tsx src/pages/team/mailbox_helpers.test.ts src/pages/team_page.smoke.test.tsx`
- `cd web && npm run test -- vite.config.test.ts src/pages/team/team_conversation_viewport.test.ts src/pages/team/mailbox_helpers.test.ts src/pages/team_panels.test.tsx`
- `cd web && npm run test -- vite.config.test.ts src/agents_route_shell.test.tsx src/components/use_agents_workbench_panel.test.tsx src/app.runtime_effects.test.tsx src/app.route_shell.test.tsx`
- `cd web && npx vitest run vite.config.test.ts src/agents_route_shell.test.tsx src/components/use_agents_workbench_panel.test.tsx src/app.runtime_effects.test.tsx src/app.route_shell.test.tsx --coverage.enabled --coverage.provider=v8 --coverage.reporter=text --coverage.include=src/components/agents_route_shell.tsx --coverage.include=src/components/use_agents_workbench_panel.ts --coverage.include=src/app.tsx`
- `cd web && npm run lint -- --ignore-pattern dist-debug --ignore-pattern dist-debug-current`
- `cd web && npm run build`
- `make build-web`

## MCP Notes

- live root page still loads cleanly without eager agents workbench hydration when no agent is selected
- live Team page now returns to `ONLINE · SSE CONNECTED` after reload without the earlier catalog-refresh storm
- Team page previously still showed periodic `/api/agents` and `/permissions` traffic from root-workbench background effects; this change gates those effects to `/`
- hidden member lookups should now stop the immediate duplicate `api/agents/:id` wave while preserving later revalidation
- local production builds no longer show the earlier eager Team-route coupling:
  - `route-teams` no longer imports `route-agents-workbench`
  - `route-teams` now reaches Team member ACP through the route-local `team_member_acp_panel` bridge plus `route-acp-shared`
  - `route-rich-text-shared` remains the Team markdown boundary
  - `route-teams` now imports `route-mantine-inputs` instead of `route-agents-debug`
  - `route-agents-debug` is reduced to the lazy ACP Debug view only
- live MCP after deploying the latest build to `agenthub.hawkingrei.com` confirms the same split:
  - first-load Team requests include `route-teams`, `route-rich-text-shared`, `route-acp-shared`, and `route-mantine-inputs`
  - first-load Team requests do not include `route-agents-workbench` or `route-agents-debug`
  - opening a Team member ACP workspace still lazy-loads `team_member_acp_panel` / `route-teams-agent-acp`, and `route-agents-debug` only appears after the user explicitly enters the `Debug` tab
  - the Team page still returns to `ONLINE · SSE CONNECTED` after reload, and the only visible console noise remains the existing `404` resources
- latest live MCP follow-up after the coverage/chunk cleanup still shows:
  - root page first-load requests remain limited to `index`, `route-agents`, `route-acp-shared`, `route-app-shared`, CSS/runtime assets, and the root API calls
  - root page still does **not** fetch `route-agents-workbench` or `route-agents-debug`
  - Team page remains `ONLINE · SSE CONNECTED` and still does **not** fetch `route-agents-workbench` / `route-agents-debug` on first load

## Follow-up

- if more Team-route reduction is needed later, focus on shrinking `route-mantine-inputs` rather than reopening the old `route-agents-debug` boundary
