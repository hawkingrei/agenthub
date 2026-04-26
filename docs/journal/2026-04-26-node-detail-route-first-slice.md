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
