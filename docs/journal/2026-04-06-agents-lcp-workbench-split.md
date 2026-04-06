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
- Moved the remaining ACP workbench runtime wiring out of the root shell and into a lazy `web/src/components/agents_workbench.tsx` boundary:
  - `useAcpConversation`
  - ACP runtime metrics and permission-history jump state
  - `OutputBody`
  - `InputDock`
- Split reusable ACP helpers into route-local modules so the root shell no longer keeps workbench-only helpers in its static graph:
  - `web/src/components/acp_input_dock_clearance.ts`
  - `web/src/components/acp_conversation_cache_stats.ts`
- Pulled the live-output routing and normalization helpers into `web/src/app_live_output.ts` so `web/src/app.tsx` no longer mixes route chrome with the SSE/live-batch plumbing in the same file.
- Pulled viewport/layout sync helpers and global permission-poll scheduling/count helpers into focused modules:
  - `web/src/app_viewport.ts`
  - `web/src/app_permission_polling.ts`
  This keeps `web/src/app.tsx` on route composition instead of mixing UI shell code with low-level browser sync and polling utilities.
- Continued the ACP workbench split inside `web/src/components/agents_workbench.tsx` by extracting:
  - `web/src/components/agents_workbench_metrics.ts` for runtime/cache/conversation metric assembly
  - `web/src/components/use_agents_permission_jump.ts` for the permission-history jump retry state machine
  This keeps the workbench component focused on view composition instead of mixing hook scheduling and pure metric shaping into the same file.
- Continued the workbench decomposition by splitting the remaining ACP panel/input-dock state wiring out of `web/src/components/agents_workbench.tsx` into:
  - `web/src/components/agents_workbench_types.ts`
  - `web/src/components/use_agents_workbench_panel.ts`
  This leaves `AgentsWorkbench` as a thin render shell while the hook owns ACP conversation/debug prop shaping and dock-clearance coordination.
- Continued the root-shell decomposition by moving the authenticated agents workspace markup out of `web/src/app.tsx` into a dedicated `web/src/components/agents_route_shell.tsx` view/wrapper pair. The new shell owns:
  - `AgentsPanel`
  - output header placement
  - splitter chrome
  - lazy `AgentsWorkbench` mounting plus fallback/error-boundary wrapping
  while `web/src/app.tsx` now only assembles shell props and route state.
- Pinned shared shell utilities (`scroll.ts`, `html_escape.ts`) back to `route-agents` so they stop dragging the workbench chunk into the route entry.
- Removed the nested lazy `AcpDebug` slot inside `web/src/components/acp_panel.tsx`; the panel now uses the already split `route-agents-debug` boundary directly instead of keeping an extra preload edge inside the workbench chunk.
- Short-circuited idle-root ACP parsing: when no agent is selected, the root shell now reuses a shared empty ACP view instead of rebuilding `buildAcpView(...)` from cached ACP lines.
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

Follow-up local build after moving ACP wiring into the lazy workbench boundary:

- `route-agents-D-pnEQUM.js`: `193.53 kB` (`57.57 kB` gzip)
- `route-agents-workbench-Cbgcgg-b.js`: `1,128.20 kB` (`382.39 kB` gzip)
- `route-agents-debug-UwyFCUWm.js`: `56.41 kB` (`16.18 kB` gzip)
- `agents_workbench-CTJz5HFq.js`: `0.08 kB` (`0.09 kB` gzip)

The primary route shell stays roughly flat in size, but it now keeps the ACP view builder and workbench runtime state entirely off the idle-root first-load path.

Follow-up local build after splitting root shared helpers out of Team/workbench chunks:

- `route-app-shared-BJ2pA2Xz.js`: `6.94 kB` (`2.03 kB` gzip)
- `route-agents-debug-loader-CcoHWXaF.js`: `2.68 kB` (`1.20 kB` gzip)
- `route-agents-Dl48vsmO.js`: `408.96 kB` (`87.77 kB` gzip)
- `route-teams-nn-LI9z8.js`: `623.67 kB` (`117.38 kB` gzip)
- `route-agents-workbench-CjUAlWZj.js`: `1,753.39 kB` (`467.25 kB` gzip)

Most importantly, the root entry chunk now statically imports only:

- `route-agents`
- `route-app-shared`
- `route-agents-debug-loader`

It no longer statically imports `route-teams` or `route-agents-workbench` JS on `/`.

## Validation

Commands run locally:

```bash
cd web && npm run test -- vite.config.test.ts src/app.permission_scope.test.ts src/acp_panel.test.tsx src/create_agent_modal.test.tsx src/permission_modal.test.tsx src/components/agent_node_section.test.tsx
cd web && npm run test -- src/agents_workbench.test.tsx src/output_body.test.tsx src/acp_debug.test.tsx
cd web && npm run test -- src/agents_route_shell.test.tsx src/app.permission_scope.test.ts src/pages/team_page.smoke.test.tsx src/agents_workbench.test.tsx
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
- Final live verification on `https://agenthub.hawkingrei.com/` after deploying the lazy-workbench boundary showed the root route no longer eagerly fetched the ACP workbench chunk when it rendered `No agent selected`:
  - first-load network stayed at `17` requests and included `/`, `route-agents`, `route-auth`, `/api/agents`, `/api/auth/status`, and `/api/settings/defaults`
  - first-load network did **not** fetch `route-agents-workbench` or the `agents_workbench` bridge chunk
  - Chrome DevTools performance trace reported `LCP 558 ms`, `TTFB 442 ms`, `render delay 116 ms`, and `CLS 0.00`
  - the only remaining console noise was the pre-existing `favicon.ico 404`
- Final follow-up live verification after moving the lightweight admin auth gate and `push.ts` out of the auth route chunk showed the idle root shell no longer eagerly fetched `route-auth` either:
  - first-load network dropped to `16` requests
  - first-load network included `/`, `index`, `route-agents`, root CSS/runtime assets, `/api/agents`, `/api/auth/status`, and `/api/settings/defaults`
  - first-load network did **not** fetch `route-auth` or `route-agents-workbench`
  - Chrome DevTools performance trace reported `LCP 484 ms`, `TTFB 338 ms`, `render delay 146 ms`, and `CLS 0.00`
  - console remained clean aside from the existing `favicon.ico 404`
- Latest local non-minified build after the chunk-boundary cleanup showed the same entry-path shape before deployment:
  - `index.html` only preloads `rolldown-runtime` and `route-agents`
  - root entry JS imports `route-agents`, `route-app-shared`, and the tiny `route-agents-debug-loader`
  - root entry JS no longer imports `route-teams` or `route-agents-workbench`
  - `route-app-shared` now owns the root+team shared live-output/event-polling helpers instead of letting Rollup fold them into the Team route chunk
- Follow-up live regression after the `AgentsWorkbench` hook split still shows the visible behavior intact:
  - `https://agenthub.hawkingrei.com/` renders `No agent selected` without eagerly fetching `route-agents-workbench`
  - `https://agenthub.hawkingrei.com/teams/...` stays `ONLINE · SSE CONNECTED`
  - the only console noise remains the existing `favicon.ico 404`
- Follow-up live regression after extracting `AgentsRouteShell` out of `web/src/app.tsx` still shows the same visible behavior:
  - root page remains `No agent selected` with no eager `route-agents-workbench` request
  - Team page remains `ONLINE · SSE CONNECTED`
  - console noise is unchanged (`favicon.ico 404` on `/`, two existing `404`s on `/teams/...`)
- Follow-up route-shell decomposition extracted the remaining `AgentsRouteShell` prop assembly out of `web/src/app.tsx` into `web/src/components/agents_route_shell_props.ts`, and memoized `AgentsRouteShell`/`AgentsRouteShellView` so unrelated root rerenders stop rebuilding the shell prop bags by default.
- Follow-up modal-shell cleanup extracted the remaining create-agent / node-section / permission-modal prop assembly into `web/src/components/agents_route_modal_props.ts`, so `web/src/app.tsx` no longer builds those large modal prop bags inline during every root render.
- Follow-up root-view extraction moved the authenticated agents shell and login-form JSX out of `web/src/app.tsx` into `web/src/components/agents_root_page.tsx`, so the root route file now stays focused on route/state orchestration instead of rendering the entire header/auth/modal tree inline.

## Follow-up

- After merge, record the push/PR CI run IDs against `docs/todo.md` for the `agents` lazy-split verification item.
- If another LCP pass is needed after merge, the next highest-value target is still `web/src/app.tsx` decomposition and further ACP workbench render-path trimming rather than more network gating.
- The next frontend decomposition target after this pass is still the remaining ACP workbench/render plumbing, but the route shell no longer owns viewport-sync or permission-poll helper code directly.
