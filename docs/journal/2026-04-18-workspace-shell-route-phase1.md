# Workspace Shell Route Phase 1

- Date: 2026-04-18

## Summary

Begin the unified workspace-shell rollout by changing the top-level Agents workbench from an
`Agents`-named shell toward a canonical `Workspace` shell, while preserving the existing Team and
Agent inner surfaces.

## Changes

- Add `/workspace` as a canonical alias for the existing Agent workbench shell.
- Update route-selection helpers so the root workbench aliases (`/` and `/workspace`) share the
  same route kind and post-auth redirect behavior.
- Update the workbench header menu to use `Workspace` as the primary shell entry instead of
  `Agents`.

## Validation

- `cd web && npm run test -- vite.config.test.ts src/workbench_header_menu.test.tsx src/app_route_selection.test.ts src/app.route_auth.test.ts`
- Chrome DevTools MCP should confirm that the top menu now renders `Workspace` and still exposes
  `Teams` as a sibling shell entry.

## 2026-07-19 Follow-up

- Added Team workspace subpath parsing for channel, channel thread, channel task, task, and member
  deep links under `/workspace/teams/:team_id/...`.
- Added an explicit canonical Team workspace subpath builder for gradual caller migration without
  changing the legacy query-string builder globally.
- Migrated the channel thread open/close navigation and Agent Nodes member drill-down links onto
  canonical Team workspace subpaths. Channel-local profile links were initially left on the legacy
  query builder and later migrated to canonical channel profile paths in this follow-up.
- Migrated TeamPage channel route canonicalization, stale task cleanup, sidebar channel selection,
  channel creation, channel deletion, and sidebar Team switching onto the same canonical subpath
  helper.
- Migrated TeamPage member workspace navigation onto canonical member subpaths.
- Migrated TeamPage shell lens navigation onto the shared canonical subpath builder, and renamed the
  local legacy-query builder import so compatibility-only route state remains intentionally
  query-string-only instead of being confused with canonical workspace subpaths.
- Centralized Team member workspace tab parsing in the exported route helper so canonical member
  subpaths and legacy `tab=` query state keep one query-first precedence rule.
- Moved Team route parsing and path construction helpers from the large `TeamPage` component module
  into `web/src/pages/team/team_route_helpers.ts`, so later shell reuse can depend on a focused
  route surface instead of importing page component internals.
- Moved route-helper expectations into `web/src/pages/team/team_route_helpers.test.ts`, leaving the
  larger `team_page.helpers.test.ts` focused on page-specific state and action helpers.
- Updated Team conversation, thread, and workbench container path construction to use the Team route
  helper module instead of importing Team path builders directly from the global app route module.
- Updated TeamPage smoke expectations for Kanban "Open conversation" actions to assert canonical
  `/workspace/teams/:team_id/channels/.../tasks/:task_id` URLs rather than the older query-string
  route shape.
- Migrated the Team channel Playwright helper fallback to the canonical Team subpath builder, so
  sidebar subject reveal and direct channel recovery no longer construct channel workspace state by
  mutating `?lens=` / `?channel=` query parameters.
- Updated Team channel browser coverage to exercise canonical
  `/workspace/teams/:team_id/channels/all/threads/:message_id` direct links and to assert
  non-default channel selection with `/channels/:channel_id` URLs.
- Updated the Team layout browser contract test to use canonical channel/task and channel/thread
  subpaths instead of mutating `?channel=...&task=...&thread=...` query state for split-pane
  validation.
- Updated the Team agent-loop profile tests to open member ACP workspaces through canonical
  `/workspace/teams/:team_id/members/:member_id/thread` subpaths instead of the legacy
  `?lens=members&member=...&tab=thread` builder.
- Updated TeamPage smoke coverage for normal channel restore, close-thread, and view-in-channel
  flows to start from canonical channel/thread subpaths; compatibility-only tests still cover old
  query strings where query-first behavior or redundant query cleanup is the explicit assertion.
- Re-exported Team route parsing through the Team route helper module and moved the Team route
  container, Team agent-loop tests, and Team channel/layout Playwright helpers onto that Team-owned
  route surface instead of importing Team path semantics directly from the global app route module.
- Re-exported Team route types and Team workspace-lens resolution through the Team route helper
  module, then moved TeamPage, Team workspace context/container types, Team sidebar types, and Team
  view-model/header helper types onto that Team-owned facade.
- Moved active Team workspace-lens fallback from the Team view-model into the Team route helper, so
  route-first precedence, Team-tab fallback, and deprecated `search` compatibility are tested beside
  the rest of the Team route semantics.
- Moved Team sidebar subject-pane resolution into the Team route helper, preserving the route-lens
  first rule for `tasks` and `members`, the Team-tab fallback for agent-focused tabs, and the
  deprecated `search` fallback to the channel pane.
- Added explicit Team route helpers for channel-scoped profile panel open/close paths, then moved
  the profile panel onto canonical channel subpaths while keeping legacy `member=` query parsing as
  a compatibility override.
- Added a named Team member workspace path helper and moved Agent Nodes Team detail/member
  drill-down links plus TeamPage member navigation onto the Team route facade.
- Migrated the remaining TeamPage channel-baseline navigation off the legacy query builder, leaving
  direct `buildTeamWorkspacePath` usage only inside the Team route helper and its compatibility
  tests.
- Updated the stable unified workspace IA contract to make `team_route_helpers.ts` the Team-owned
  route facade for Team route parsing, path construction, route types, workspace-lens resolution,
  active-lens fallback, Team-tab mapping, sidebar subject-pane resolution, and compatibility-only
  legacy query construction.
- Added named canonical Team route helpers for channel baselines, channel threads, channel-scoped
  tasks, Team task paths, Team member workspaces, and lens-preserving Team switches, then moved
  TeamPage, Team conversation/thread containers, Team smoke tests, Team agent-loop tests, and Team
  browser helpers off direct low-level canonical builder calls.
- Added a Team route-selection snapshot helper so TeamPage consumes one facade-owned parse result
  for workspace lens, Team tab, channel, thread root, selected task, and selected member instead of
  independently deriving each route fact in the page component.
- Added named Team selector, deprecated search-lens compatibility, and tab-only compatibility path
  helpers, then moved TeamPage selector navigation and Team browser helper fallbacks off hand-written
  Team route strings and direct Team query-parameter mutation.
- Added a Team route split helper for callers that receive a facade-built path but must pass
  `pathname` and `search` separately, then moved more TeamPage smoke entrypoints for canonical
  channel, channel-task, and task-lens behavior onto named Team route helpers.
- Added a static Team route facade boundary test that scans Team production sources plus Agent Nodes
  Team drill-down code and fails if global Team route parser/builder symbols leak outside
  `team_route_helpers.ts`.
- Extended the Team route facade boundary test to scan Team E2E sources and reject direct
  `page.goto("/teams...")` / `page.goto("/workspace/teams...")` navigation, then moved the shared
  E2E Team selector entrypoint and channel-thread deep link onto named Team route helpers.
- Extended the same direct-navigation guard to the shell menu and global workspace-lens selector,
  then moved both production Team selector entrypoints onto `buildTeamSelectorPath()`.
- Removed the `resolveTeamRoute` re-export from the app module and added a boundary check that keeps
  app-level exports from re-exposing Team route symbols.
- Moved the route-lens to Team-tab mapping into the same Team route helper module, including the
  `search` compatibility rule that keeps deprecated shell search from becoming a Team content tab.
- Aligned the shared workspace lens label for `members` with the canonical `Members` language
  instead of the older Agent-specific label.
- Extracted the Team workspace header visibility rule into a tested helper so deeper shell reuse
  can keep `search` routed through the channel surface while preserving task-first suppression of
  the chat workspace header.
- Moved the deprecated `search` lens normalization used by Team workspace header decisions into the
  Team route helper module, keeping lens compatibility semantics beside the rest of the Team-owned
  facade instead of in the UI content component.
- Extracted Team active workspace-lens resolution into a tested helper and aligned the Team
  view-model test expectations with canonical `Members` language.
- Aligned the shell-level Search placeholder copy with canonical `Channels`, `Tasks`, and
  `Members` language while leaving Team sidebar search behavior unchanged.
- Kept query-string deep links compatible and query-first during migration, so existing links such
  as `?channel=...`, `?thread=...`, `?task=...`, `?member=...`, and `?tab=...` still override
  path-derived state.
- Preserved the v1 route guardrail that `thread` is not a top-level shell lens: thread path state is
  only recognized under the channel surface, and unknown `/workspace/teams/:team_id/thread` paths
  still resolve to the channel baseline.
- Tightened the Team route facade boundary guard so `TeamPage` and `AgentNodesWorkbench` are no
  longer allowlisted for direct global Team route symbols, and expanded direct Team navigation
  scanning across the Team page, Team submodules, node workbench, workspace shell lens entrypoints,
  and Team E2E helpers.
- Extended the production-source navigation guard to reject literal Team `href` / `to` links in the
  same Team page, Team submodule, node workbench, and shell lens entrypoint surfaces, keeping
  browser-visible Team URLs behind named facade builders.
- Extended the Team route facade boundary guard so production sources cannot import the positional
  `buildCanonicalTeamSubpath` helper from the facade. Canonical Team navigation should stay behind
  named builders such as channel, task, member, selector, and compatibility helpers.
- Migrated channel-scoped member profile panel open paths to
  `/workspace/teams/:team_id/channels/:channel_id/members/:member_id` and task-scoped profile
  panels to `/workspace/teams/:team_id/channels/:channel_id/tasks/:task_id/members/:member_id`.
  Profile close paths now return to the canonical channel baseline instead of the legacy
  `?channel=` compatibility URL. Query `member=` continues to take precedence when old links are
  parsed during migration.
- Added shared `WorkspaceSectionShell`, `WorkspaceContentStack`, and `WorkspaceSplitPaneLayout`
  primitives, then moved the Team workspace header wrapper, selected-team/body stack wrappers, and
  channel/thread primary-secondary layout onto them. Deeper shell reuse now owns the base section
  chrome, overflow, spacing variants, and secondary pane dock sizing while Team keeps only its
  domain header/content decisions and compact agent-workspace signals.

Focused validation for this follow-up:

- `cd web && npm exec vitest -- run src/pages/team_page.helpers.test.ts src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm exec vitest -- run src/app_route_selection.test.ts src/pages/team_page.helpers.test.ts src/components/agent_nodes_workbench.test.tsx`
- `cd web && npm exec vitest -- run src/components/workspace_lens_items.test.ts`
- `cd web && npm exec vitest -- run src/pages/team/team_workbench_content.test.tsx`
- `cd web && npm exec vitest -- run src/pages/team/use_team_workspace_view_model.test.tsx`
- `cd web && npm exec vitest -- run src/components/workspace_lens_placeholder.test.tsx src/routes/agents_route_container.test.tsx`
- `cd web && npm exec vitest -- run src/pages/team/team_route_helpers.test.ts src/pages/team_page.helpers.test.ts src/app_route_selection.test.ts src/pages/team/use_team_workspace_view_model.test.tsx`
- `cd web && npm exec vitest -- run src/pages/team/team_route_helpers.test.ts src/pages/team_page.helpers.test.ts src/pages/team/team_thread_pane.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm exec vitest -- run src/pages/team_page.agent_loop.test.tsx src/pages/team/team_route_helpers.test.ts src/app_route_selection.test.ts`
- `cd web && npm exec vitest -- run src/pages/team_page.smoke.test.tsx src/pages/team/team_route_helpers.test.ts src/app_route_selection.test.ts`
- `cd web && npm exec vitest -- run src/pages/team/team_route_helpers.test.ts src/routes/team_route_container.test.tsx src/pages/team_page.agent_loop.test.tsx src/app_route_selection.test.ts`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`
- `cd web && npm exec vitest -- run src/pages/team/team_route_helpers.test.ts src/pages/team/use_team_workspace_view_model.test.tsx src/pages/team/team_workbench_content.test.tsx src/pages/team_page.helpers.test.ts`
- `cd web && npm exec vitest -- run src/pages/team/team_route_helpers.test.ts src/pages/team_panels.test.tsx src/pages/team/use_team_workspace_view_model.test.tsx`
- `cd web && npm exec vitest -- run src/pages/team/team_route_helpers.test.ts src/pages/team_panels.test.tsx src/pages/team/team_workbench_content.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm exec vitest -- run src/pages/team/team_route_helpers.test.ts src/components/agent_nodes_workbench.test.tsx src/pages/team_page.smoke.test.tsx src/pages/team_page.agent_loop.test.tsx`
- `cd web && npm exec vitest -- run src/pages/team_page.smoke.test.tsx src/pages/team/team_route_helpers.test.ts src/app_route_selection.test.ts`
- `cd web && PLAYWRIGHT_SYSTEM_CHROME=1 npx playwright test tests/e2e/team_page_channels.e2e.ts --project=system-chrome`
- `cd web && PLAYWRIGHT_SYSTEM_CHROME=1 npx playwright test tests/e2e/team_page_layout.e2e.ts --project=system-chrome`
- `cd web && PLAYWRIGHT_SYSTEM_CHROME=1 npx playwright test tests/e2e/team_page_channels.e2e.ts tests/e2e/team_page_layout.e2e.ts --project=system-chrome`
- `cd web && npm exec vitest -- run src/pages/team/team_route_helpers.test.ts src/pages/team_page.helpers.test.ts src/pages/team/use_team_workspace_view_model.test.tsx src/pages/team/team_workbench_content.test.tsx src/routes/team_route_container.test.tsx`
- `cd web && npm exec vitest -- run src/pages/team/team_route_helpers.test.ts src/pages/team/team_workbench_content.test.tsx`
- `cd web && npm exec vitest -- run src/pages/team_page.smoke.test.tsx src/pages/team/team_route_helpers.test.ts`
- `cd web && npm exec vitest -- run src/pages/team/team_route_boundary.test.ts src/pages/team/team_route_helpers.test.ts`
- `cd web && npm exec vitest -- run src/pages/team/team_route_boundary.test.ts src/pages/team/team_route_helpers.test.ts src/routes/use_workspace_route_state.test.tsx src/workbench_header_menu.test.tsx`
- `cd web && npm exec vitest -- run src/pages/team/team_route_boundary.test.ts src/app.route_auth.test.ts src/pages/team/team_route_helpers.test.ts`
- `cd web && npm exec vitest -- run src/pages/team/team_route_boundary.test.ts`
- `cd web && npm exec vitest -- run src/components/layout/workspace_section_shell.test.tsx src/pages/team/team_workbench_content.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `git diff --check`
