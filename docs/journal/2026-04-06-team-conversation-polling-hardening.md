# Team Conversation Polling Hardening

## Summary

- reduced repeated Team task detail fetches when a selected thread still has no latest run
- stopped re-fetching thread detail on a fixed cooldown after the UI has already confirmed `latest_run = null`; the selected conversation now treats a cached `null` run id as stable instead of retrying forever
- aligned Team conversation SSE fallback so 4s polling only runs when SSE is unavailable or disconnected
- moved Team member ACP auto-sync to the same `SSE first + disconnect fallback poll` model as the Agents workbench
- gated root Agents workbench background SSE and permission polling so it no longer runs behind Team routes
- added a cooldown to Team member agent-card backfill so cached hidden members are not revalidated on every rerender
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
- updated Vite chunk routing so Team-local markdown and Team member ACP have their own named boundaries (`route-shared-rich-text`, `route-teams-agent-acp`) instead of piggybacking silently on the default route graph
- delayed clearing a selected non-shared thread until task refresh has actually settled:
  - keep the selected thread while `tasksLoading` is still true
  - keep the selected thread when `selectedConversationDetail.task` still provides a fallback task record
  - only clear after the refreshed task list confirms the thread is gone
- aligned Team Tailwind theme variables with the semantic tokens already referenced by the route-level classes and tests so route-local styling no longer depends on stale theme drift between `tailwind.config.cjs` and `tailwind.css`

## Validation

- `cd web && npm run test -- src/pages/team_panels.test.tsx src/pages/team/use_team_conversation_actions.test.tsx src/pages/team/use_team_conversation_effects.test.tsx src/pages/team/use_team_member_acp_effects.test.tsx src/ui/primitives.test.tsx`
- `cd web && npm run test -- vite.config.test.ts src/input_dock_keyboard.test.ts src/components/thread_rich_text.test.tsx src/pages/team/mailbox_helpers.test.ts src/pages/team_page.smoke.test.tsx`
- `cd web && npm run lint -- --ignore-pattern dist-debug --ignore-pattern dist-debug-current`
- `cd web && npm run build`
- `make build-web`

## MCP Notes

- live root page still loads cleanly without eager agents workbench hydration when no agent is selected
- live Team page now returns to `ONLINE · SSE CONNECTED` after reload without the earlier catalog-refresh storm
- Team page previously still showed periodic `/api/agents` and `/permissions` traffic from root-workbench background effects; this change gates those effects to `/`
- hidden member lookups should now stop the immediate duplicate `api/agents/:id` wave while preserving later revalidation
- live `agenthub.hawkingrei.com` still serves an older bundle at the time of this note, so DevTools network traces there continue to show `route-agents-debug` and `route-agents-workbench` on the Team route; that is a deployment lag, not the local build result
- local production builds no longer show the earlier eager Team-route coupling:
  - `route-teams` now resolves through `vendor-mantine`, `route-ui-shared`, `route-teams-rich-text`, and `route-teams-agent-acp`
  - `rg -n "route-agents-(debug|workbench)" web/dist/assets/route-teams-*.js web/dist/assets/route-teams-agent-acp-*.js` returns no matches
- the follow-up split switched Vite chunk routing from the ineffective Rollup-style `manualChunks` path to Rolldown `codeSplitting.groups`, which is the change that finally severed the Team route from the agents workbench/debug chunks

## Follow-up

- redeploy and re-check `agenthub.hawkingrei.com` with DevTools so the live Team route evidence catches up with the local build result
- keep the TODO item open until post-deploy verification confirms `/teams` no longer fetches `route-agents-workbench` or `route-agents-debug` on first load
