# Team UI Shell And Bundle Refinement

## Summary

- simplified the Team selector into a slimmer chooser that more closely follows the Slock information hierarchy while keeping a more neutral Notion-like visual tone
- reduced Team and Agent ACP shell chrome so the main content area gets more width and less repeated metadata
- aligned the Team channel and ACP viewport/rich-text foundations through shared `thread_viewport` and `thread_rich_text` building blocks
- moved Team channel read state into a bottom-right radial indicator with hover details for read/unread recipients
- improved initial web bundle behavior by splitting route-level chunks and removing `vendor-markdown` from the initial preload chain

## Details

### Team and ACP UI

- narrowed the Teams sidebar/workspace ratio to match the Slock-style "narrow rail, wide content" layout
- kept the color system closer to Notion-style neutrals instead of adopting the brighter Slock palette
- made ACP input docking behave like the main `/agents` workbench so the composer stays pinned at the bottom of the panel
- reduced default ACP content density by collapsing larger payload sections and trimming preview sizes

### Channel read-state treatment

- replaced text `N seen` actions with a hoverable radial progress marker rendered in the bottom-right corner of each message bubble
- kept pending delivery in the same bottom-right status position so message delivery/read feedback uses one consistent affordance

### Bundle loading

- lazy-loaded `Join`, `Admin`, and `Teams` pages through route-level chunk loading
- extracted `escapeHtml` into a lightweight helper so route-independent code no longer imports the heavy markdown renderer path
- moved highlight.js theme CSS out of the entry bundle
- changed Vite preload filtering so `route-auth`, `route-teams`, and `vendor-markdown` are no longer preloaded from `index.html`
- split the Team detail workbench into `route-teams-workbench` so the selector/sidebar shell no longer imports the detail body directly
- moved Team member agent backfill into an app-level helper module so the global agents hook no longer pulls Team route UI code into the entry path

### Review and CI follow-ups

- made lazy-route loader errors name the missing loader explicitly to improve production debugging
- hardened `thread_rich_text` fallback behavior by catching lazy markdown asset load failures, refreshing cache recency on hit, and splitting plain-text paragraphs on blank lines
- kept Team ACP refresh visible even before a member session exists so operators can poll for newly-started sessions
- restored input dock send-button tap targets to the sizes enforced by the web asset and Playwright layout checks while keeping the surrounding shell visually thin
- updated Team E2E helpers to follow the slimmer ACP shell and renamed `More` workspace actions entry
- added a shared `preloadThreadMarkdownAssets()` test hook so static ACP render tests can warm the lazy markdown renderer without pulling `vendor-markdown` back into the entry bundle
- rendered lazy fold bodies during server-side/static test markup so ACP SSR snapshots can still assert nested payload structure while browser behavior stays collapsed-by-default
- updated ACP and app helper tests to match the new agents sidebar width policy and the slimmer tail-window defaults for long payload previews
- aligned Team smoke/E2E checks with the slimmer selector copy and ACP shell readiness signal, so CI no longer waits on the old `Conversation` tab before considering Team ACP ready
- restored tablet-sized input history/interrupt chips to a 26px minimum while keeping the mobile override at 24px, matching the dock layout Playwright checks without bringing back the thicker desktop shell
- updated the Team runtime-badge E2E helper to fall back to the slimmer inline badge text when the status is no longer exposed via a dedicated `role="status"` node
- added focused unit coverage for `thread_rich_text` cache/fallback behavior and `thread_viewport` jump/stick helpers to lift diff coverage on the shared Team/ACP primitives
- moved the Team runtime-badge E2E expectation to the selected-team menu, which is where the slimmer shell now exposes runtime status
- added helper-focused coverage for `team_page`, `team_sidebar`, and `agent_node_section` so diff coverage follows the newly introduced chooser/layout helpers instead of only the large panel integration tests
- replaced invalid Tailwind slash-opacity shorthands with explicit arbitrary opacity values so the thinner Team shell styling survives production builds
- kept `Pending delivery` separate from seen-progress rendering so unread outbound messages do not appear as a 0% read indicator
- marked Machines & Agents selection buttons with `aria-pressed` so the chooser state remains accessible in the slimmer node picker

## Validation

- `make build-web`
- `cd web && npx vitest run src/app.permission_scope.test.ts src/acp_conversation.test.ts src/acp_conversation_render.test.tsx src/acp_conversation.interaction.test.tsx`
- `cd web && npx vitest run src/pages/team_page.smoke.test.tsx`
- `cd web && npm run lint -- src/components/thread_rich_text.tsx src/components/acp_conversation.tsx src/acp_conversation.test.ts src/acp_conversation_render.test.tsx src/acp_conversation.interaction.test.tsx src/app.permission_scope.test.ts`
- `cd web && npm run lint -- src/pages/team_page.smoke.test.tsx tests/e2e/team_page.e2e.ts`
- `cd web && npx vitest run src/components/thread_rich_text.test.tsx src/hooks/thread_viewport.test.ts`
- `cd web && npx vitest run src/pages/team_page.helpers.test.ts src/pages/team_sidebar.helpers.test.ts src/components/agent_node_section.test.tsx src/app.route_auth.test.ts`
- `cd web && npm run test:coverage:core`
- inspected `web/dist/index.html` to confirm the initial preload set no longer includes `route-auth`, `route-teams`, or `vendor-markdown`
- `npm --prefix web run test -- vite.config.test.ts src/pages/team/use_team_member_backfill_effect.test.tsx src/pages/team/team_page_route_props.test.ts src/pages/team_page.smoke.test.tsx`
- `npm --prefix web run test -- vite.config.test.ts src/pages/team_page.smoke.test.tsx`
- `npm --prefix web run build`
- `npm --prefix web run lint`
- inspected `web/dist/index.html` and preview-served `/workspace/teams` HTML to confirm the initial preload set still excludes `route-teams`, `route-teams-workbench`, `route-teams-agent-acp`, and `route-acp-shared`
- `curl -L -s -D /tmp/agenthub-deployed-headers.txt https://agenthub.hawkingrei.com/workspace/teams -o /tmp/agenthub-deployed-teams.html`
- `rg -n "modulepreload|route-teams|route-teams-workbench|route-teams-agent-acp|route-acp-shared|assets/index" /tmp/agenthub-deployed-teams.html`
- verified the deployed `agenthub.hawkingrei.com/workspace/teams` HTML on 2026-06-12 returned `cache-control: no-cache` and an initial modulepreload set limited to `rolldown-runtime`, `route-agents`, `route-mantine-inputs`, `route-rich-text-shared`, and `vendor-mantine`; it did not preload `route-teams`, `route-teams-workbench`, `route-teams-agent-acp`, or `route-acp-shared` JavaScript
- attempted a Chrome DevTools authenticated visual spot-check for the deployed Team selector, but the DevTools transport closed before navigation; the deployed route-split evidence above is based on the live HTML response
