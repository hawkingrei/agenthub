# Node Detail Route First Slice

## Scope

- added a canonical workspace route for node inspection: `"/workspace?lens=nodes&node=<node_id>"`;
- introduced a dedicated node-detail workbench surface for root operators;
- extracted the shared node detail rendering so the existing `Agents` node-management panel can
  reuse the same `Info`, `Connect Command`, and `Agents On This Node` structure;
- kept node creation and node settings edits in the existing inline management modal.

## What Changed

- `web/src/app_route_selection.ts`
  - added `nodes` as a first-class workspace lens;
  - added `resolveWorkspaceNodeId` and `buildWorkspaceNodePath`.
- `web/src/app.tsx`
  - routes root users into a dedicated node-detail workbench when `lens=nodes`;
  - keeps `nodes` lens canonical on `/workspace` instead of agent-specific routes;
  - lets the node detail surface select nodes, open attached agents, and open create-agent flow.
- `web/src/components/agent_node_detail_shared.tsx`
  - centralized node detail rendering for `Info`, `Connect Command`, and attached-agent roster.
- `web/src/components/agent_nodes_workbench.tsx`
  - added the new node detail page shell with node roster + detail content.
- `web/src/components/agent_node_section.tsx`
  - reuses the shared detail card and links into the canonical node detail route.

## Current Limitations

- node connectivity still does not have a dedicated heartbeat model; the current slice records
  bootstrap-driven `last_seen_at`, which is useful but weaker than continuous liveness;
- the connect command is token-first and placeholder-based for the values the UI cannot derive
  today (for example full TLS/auth config);
- danger-zone separation and workspace/session attachments are intentionally deferred.

## Validation

- `cd web && pnpm exec vitest run src/app_route_selection.test.ts src/components/agent_node_section.test.tsx src/agents_route_shell.test.tsx`
- `cd web && npm run build`

## Follow-up Slice

- promoted remote node settings and danger-zone actions into the canonical node detail route so the
  detail page is no longer read-only;
- kept the existing modal management surface as a bootstrap path, but the stable object page now
  carries:
  - editable remote node settings;
  - destructive guardrails and delete action;
  - attached-agent deletion constraint copy.

### Additional Validation

- `cd web && pnpm exec vitest run src/components/agent_nodes_workbench.test.tsx src/components/agent_node_section.test.tsx src/app_route_selection.test.ts src/agents_route_shell.test.tsx`
- `cd web && npm run build`
- Chrome DevTools MCP opened `http://127.0.0.1:4173/workspace?lens=nodes&node=node-east` and
  confirmed the `Nodes` lens route activates correctly.
  The local check remained shell-only because Vite served the SPA without a backing API, so the
  page fell back to the empty-state/error path instead of real node data.

## Connect Command Contract Follow-up

- the shared node detail card now exposes:
  - copy action for the generated command;
  - explicit ownership badges for substituted values vs. manual follow-up values;
  - placeholder fallback when bootstrap token data is unavailable instead of hiding the command.
- this keeps the page aligned with the feature spec without pretending the UI knows more runtime
  config than the current API actually returns.

## Runtime Identity Follow-up

- added persisted `agent_nodes.last_seen_at` in DB init/migration paths;
- `issue_node_credential` now updates `last_seen_at` for registered remote nodes on bootstrap
  credential issuance;
- node detail now prefers persisted `last_seen_at` over indirect attached-agent activity when
  rendering runtime summary:
  - `Recently Seen`
  - `Seen Earlier`
  - fallback to `Agent Activity Detected`
  - fallback to `Unverified`
- this remains a lightweight signal only; it does not claim ongoing node liveness after bootstrap.

## Team Surface Follow-up

- extended team-local node presence beyond ACP and Member Console into `Team Snapshot`;
- `TeamOverviewPanel` member rows now show a lightweight `node=<node_id>` summary and link into the
  canonical global node detail route when the runtime attachment is known;
- `TeamPage` now derives a member-to-node lookup from resolved team member agents and passes that
  lookup into the overview panel instead of duplicating node inference in the view itself.
- refined the overview row treatment so local `main` attachments and remote node attachments are
  visually distinct (`local` vs `remote`) without turning the row into a full machine detail card.

### Additional Validation

- `cd web && pnpm exec vitest run src/pages/team_panels.test.tsx src/pages/team_member_acp_panel.test.tsx`
- `cd web && npm run build`

## Global-to-Team Navigation Follow-up

- `Teams Using This Node` now acts as a real jump surface instead of a read-only roster;
- node detail team cards link directly into the canonical Team workspace route
  (`/workspace/teams/<team_id>`) so operators can pivot from a global machine/object page back
  into the relevant team context without re-searching;
- the new links preserve browser-native modified-click and open-in-new-tab behavior while still
  using SPA navigation for plain left-clicks.

### Additional Validation

- `cd web && npm run lint`
- `cd web && pnpm exec vitest run src/app_route_selection.test.ts src/components/agent_nodes_workbench.test.tsx`

## Shared Menu Entry Follow-up

- added a root-only `Machines` entry to the shared workbench header menu;
- this keeps Team workspace lens bars team-local (`Channels / Tasks / Members / Search`) while
  still giving operators one stable way to jump into the global machine/node surface from any
  workspace shell;
- the menu entry reuses the canonical `/workspace?lens=nodes` route instead of introducing another
  machine-specific navigation path.

### Additional Validation

- `cd web && npm run lint`
- `cd web && pnpm exec vitest run src/workbench_header_menu.test.tsx src/workbench_header_menu.interaction.test.tsx src/components/workspace_shell_header.test.tsx`

## Team Menu Entry Follow-up

- added a root-only `Machines` action to the Team sidebar controls menu;
- this gives Team workspaces a visible global-machine escape hatch without promoting `Machines`
  into the Team-local primary tabs;
- Team shell now exposes the global machine surface in two consistent places:
  - shared workbench header menu;
  - Team-specific controls menu.
- when a Team member is already in focus, the Team controls menu now also exposes a contextual
  `Current Machine` shortcut so operators can jump straight to that member's attached node instead
  of first landing on the generic machine roster.

### Additional Validation

- `cd web && npm run lint`
- `cd web && pnpm exec vitest run src/pages/team_panels.test.tsx src/workbench_header_menu.test.tsx src/workbench_header_menu.interaction.test.tsx`

## Team Member Drill-down Follow-up

- added a shared `buildTeamWorkspacePath(...)` helper to the route-selection layer so Team member
  deep links can be assembled once and reused outside `team_page.tsx`;
- upgraded `Teams Using This Node` member badges into compact drill-down capsules:
  - the primary member label routes to Team member ACP;
  - a secondary `Console` action routes to Team member console;
  - both preserve browser-native modified-click behavior while still using SPA navigation for plain
    left-clicks;
- this lets the global node detail surface pivot all the way back into the exact Team member
  runtime context instead of stopping at Team detail.

### Additional Validation

- `cd web && npm run lint`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && pnpm exec vitest run src/app_route_selection.test.ts src/components/agent_nodes_workbench.test.tsx`

## Node Detail UI Polish Follow-up

- restructured `Teams Using This Node` into a clearer detail hierarchy:
  - section-level summary card with aggregate metrics;
  - per-team cards with compact member/leader/worker metrics;
  - a dedicated `Member Runtime Drill-down` sub-section for `ACP / Console` entry points;
- this keeps the global node page closer to an object-detail surface and reduces the old
  all-badges-on-one-row visual flattening.

### Additional Validation

- `cd web && npm exec tsc -- --noEmit`
- `cd web && pnpm exec vitest run src/components/agent_nodes_workbench.test.tsx`
- `cd web && npm run build`

## Shared Detail Card UI Follow-up

- polished the shared `AgentNodeDetailCard` so both the canonical node workbench and the inline
  node section reuse the same stronger detail-page hierarchy;
- added a compact summary metric strip under the node header, clearer section subcopy for `Info`
  and `Connect`, and a tighter `Agents on this node` header/action layout;
- kept all field contracts stable so this slice is purely presentation/hierarchy work on top of the
  existing node identity and connect-command model.

### Additional Validation

- `cd web && npm exec tsc -- --noEmit`
- `cd web && pnpm exec vitest run src/components/agent_nodes_workbench.test.tsx src/components/agent_node_section.test.tsx`
- `cd web && npm run build`

## Root Machines Shell Empty-State Cleanup

- the root workspace machines lens was still inheriting the agent output header shell, which left
  the misleading `No agent selected` / `Select an agent to continue.` empty state above the
  machines surface;
- route wiring now passes an explicit `showOutputHeader={false}` path through
  `AgentsRootPage -> AgentsRouteShell` whenever `lens=nodes` is active;
- this keeps the root machines surface owned by the node workbench instead of a stale agent shell.

### Additional Validation

- `cd web && pnpm exec vitest run src/agents_route_shell.test.tsx src/app.route_shell.test.tsx src/components/agent_nodes_workbench.test.tsx`
- `cd web && npm exec tsc -- --noEmit`

## Team Member Thread Route Vocabulary Follow-up

- the Team member runtime surface was still exposed publicly as `Agent ACP`, even though the actual
  page reads more like a member thread/runtime detail than a raw ACP-native screen;
- canonical deep links now serialize the member runtime tab as `tab=thread` while continuing to
  accept the legacy `tab=agent_acp` alias on read;
- node-detail drill-down copy now uses `Thread` instead of `ACP`, and the Team workbench tab label
  follows the same public vocabulary;
- route-selected members now stay addressable when the Team already knows the member id from
  runtime/spec/snapshot state, instead of collapsing to the empty `Select an agent from the left
  rail to inspect its thread.` fallback during intermediate selection state.

### Additional Validation

- `cd web && pnpm exec vitest run src/app_route_selection.test.ts src/pages/team_page.helpers.test.ts src/pages/team/page_helpers.test.ts src/components/agent_nodes_workbench.test.tsx src/pages/team_panels.test.tsx src/pages/team_member_acp_panel.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
