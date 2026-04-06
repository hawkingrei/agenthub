# Agents LCP Workbench Split

## Why

The deployed `agents` route still shipped a monolithic `route-agents` chunk that pulled in the full ACP workbench subtree on first load. That kept the route entry JS large enough to pressure LCP even when the first meaningful paint only needed the agents list shell and header chrome.

## What Changed

- Moved the agents workbench body onto lazy route-local imports in `web/src/app.tsx`:
  - `OutputBody`
  - `InputDock`
- Moved `CreateAgentModal`, `AgentNodeSection`, and `PermissionModal` behind route-local lazy imports so the route shell no longer pays for their Mantine-heavy subtrees during first paint.
- Added lightweight Suspense fallbacks so the workbench shell can paint immediately while the heavy ACP/terminal subtree streams in afterwards.
- Split Vite manual chunks so the heavy workbench modules no longer get forced back into the primary `route-agents` chunk:
  - primary shell stays in `route-agents`
  - ACP/workbench subtree now lands in `route-agents-workbench`
- Split `AcpDebug` into its own lazy chunk because it is default-hidden behind the Debug tab.
- Kept shared workbench chrome/helpers in the primary agents chunk so `/` no longer needs to fetch `route-teams` just to render:
  - `WorkbenchConnectionBadge`
  - `WorkbenchHeaderMenu`
  - `acp_panel_helpers`
  - shared shell helpers such as `error_banner`, `auth_redirect`, `worktree_defaults`, and `input_history`
- Restricted full ACP permission history polling to Developer Mode + Debug tab only; normal Conversation view now polls pending approvals only.
- Short-circuited unchanged `/api/agents` and `/api/agent_nodes` refreshes in `web/src/app.tsx` so periodic refreshes do not force avoidable React rerenders.
- Stopped prefetching `agent_nodes` on the root workbench route; root users now fetch node inventory only when the create-agent modal opens.
- Stopped prefetching admin-only datasets (`safe_paths`, `devices`, `audits`, `vapid`, `admin/settings`) outside `/admin`.
- Reduced the initial ACP event page size from `200` to `80` so the first workbench conversation load no longer requests a full large-history window.
- Deferred ACP conversation auto-history backfill by `1200 ms` so `before_id` enrichment starts after first paint instead of competing directly with initial LCP.
- Stopped auto-selecting the first non-running agent on `/`; the root route now auto-mounts the heavy ACP workbench only when a running agent exists.
- Hid the agents-route input dock when no agent is selected so the idle root shell no longer pulls interaction-only workbench UI just to show the empty-state placeholder.
- Limited global pending-permission polling to active agents so idle/exited agents no longer add avoidable root-route network churn.
- Added focused regression tests in `web/vite.config.test.ts` and `web/src/app.permission_scope.test.ts` to lock the chunk routing and refresh short-circuit behavior.
- Added focused runtime tests in `web/src/app.runtime_effects.test.tsx` to lock the new admin-route gating, node-fetch gating, and smaller ACP event page budget.

## Build Delta

Production build before this split:

- `route-agents-DshAFLEj.js`: `1,349.67 kB` (`446.03 kB` gzip)

Production build after this split:

- `route-agents-DrRYyqxN.js`: `115.01 kB` (`31.42 kB` gzip)
- `route-agents-workbench-DjkdKzwv.js`: `1,124.14 kB` (`383.55 kB` gzip)
- `route-agents-debug-BDTVVnRI.js`: `114.87 kB` (`35.57 kB` gzip)

This keeps the total workbench code roughly similar, but it removes that weight from the first route shell payload so the browser can paint the agents surface sooner.

Follow-up local build after the second round of chunk routing:

- `route-agents-BDcKScdJ.js`: `194.32 kB` (`60.48 kB` gzip)
- `route-agents-workbench-BmVwoVvA.js`: `1,132.47 kB` (`387.93 kB` gzip)
- `route-agents-debug-BgIGLW2s.js`: `56.43 kB` (`16.14 kB` gzip)
- `route-agents-terminal-Da9UaUUk.js`: `0.69 kB` (`0.41 kB` gzip)
- `route-auth-CYw8x2xz.js`: `23.57 kB` (`8.31 kB` gzip)

This second round intentionally keeps more shared shell code in `route-agents` so the browser no longer has to pull `route-teams` during initial `/` rendering just to resolve common workbench chrome.

## Validation

Commands run locally:

```bash
cd web && npm run test -- vite.config.test.ts src/app.permission_scope.test.ts src/acp_panel.test.tsx src/create_agent_modal.test.tsx src/permission_modal.test.tsx src/components/agent_node_section.test.tsx
cd web && npm run lint -- --ignore-pattern dist-debug
cd web && npm run build
make build-web
```

Live verification notes:

- Before the second round, `agenthub.hawkingrei.com/` still fetched `route-teams` and `route-agents-debug` on `/`, and full ACP permission history polling showed up outside the Debug tab.
- After the code changes in this branch, local production build output no longer has `index -> route-teams` static imports; the remaining live eager fetches were from an older deployed build, not the current local output.
- Chrome DevTools MCP later recovered and confirmed the remaining live over-polling was down to `/api/agents` plus `permissions?status=pending`; the full `/permissions` history call no longer appeared outside Developer Mode + Debug.
- A later live MCP baseline on `https://agenthub.hawkingrei.com/` showed the root route still auto-selected an `EXITED` agent and mounted the ACP workbench by default. The follow-up fix in this branch removes that fallback so the idle route can stay on the lighter list-first shell unless a running agent exists.
- The next live MCP baseline still showed `No agent selected` plus a visible input dock, which kept `route-agents-workbench` eager on the root route. This branch now suppresses the dock until an agent is actually selected.
- The same idle-root baseline also showed `permissions?status=pending` polling for an exited agent. The branch now filters global pending-permission polling down to active agents only.
- Final live after-check on `https://agenthub.hawkingrei.com/` after deploying `index-9Xgk5xXY.js` showed:
  - first-load network no longer fetches `route-teams` or `route-agents-debug`
  - first-load scripts are limited to `index`, `route-agents`, `route-auth`, `route-agents-terminal`, and `route-agents-workbench`
  - observed lab LCP dropped to `1634 ms` with `TTFB 582 ms` and `render delay 1053 ms`
  - normal Conversation view still polls `permissions?status=pending`, but the full ACP permission history endpoint no longer appears outside Developer Mode + Debug
- Follow-up live traces after the next round of root-route request gating showed the primary improvement was request hygiene rather than a stable final LCP win:
  - `/admin`-only requests disappeared from the root route waterfall
  - `agent_nodes` no longer loads on first paint
  - initial ACP event requests dropped from `limit=200` to `limit=80`
  - auto-loaded `before_id` history fetches moved later, outside the first immediate mount work
  - observed lab LCP remained noisy (`~1.1 s` best trace, `~3.1 s` and `~5.8 s` on later cold traces), which means the remaining bottleneck is still render delay inside the heavy agents workbench path rather than obvious stray network requests

## Follow-up

- Deploy the current branch and re-measure `/` on `agenthub.hawkingrei.com` to verify `route-teams` / `route-agents-debug` disappear from the first-load network waterfall.
- If the remaining first-load cost is still dominated by the workbench subtree, continue by pushing more of the ACP terminal/conversation path behind interaction-driven lazy boundaries or by further delaying nonessential ACP history hydration.
