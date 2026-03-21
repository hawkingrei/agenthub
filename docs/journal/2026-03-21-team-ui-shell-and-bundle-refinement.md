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

## Validation

- `make build-web`
- `cd web && npx vitest run src/app.permission_scope.test.ts src/acp_conversation.test.ts src/acp_conversation_render.test.tsx src/acp_conversation.interaction.test.tsx`
- `cd web && npx vitest run src/pages/team_page.smoke.test.tsx`
- `cd web && npm run lint -- src/components/thread_rich_text.tsx src/components/acp_conversation.tsx src/acp_conversation.test.ts src/acp_conversation_render.test.tsx src/acp_conversation.interaction.test.tsx src/app.permission_scope.test.ts`
- `cd web && npm run lint -- src/pages/team_page.smoke.test.tsx tests/e2e/team_page.e2e.ts`
- inspected `web/dist/index.html` to confirm the initial preload set no longer includes `route-auth`, `route-teams`, or `vendor-markdown`
