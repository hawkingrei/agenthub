# Workspace Shell Phases 1-3 Convergence

- Date: 2026-04-18

## Summary

Push the unified workspace-shell rollout beyond the initial `/workspace` alias by landing one
shared shell header, canonical Team/Agent route aliases, a first Team-shell lens bar, and the
first Agent entity deep-link path.

## Changes

- Introduce a shared `WorkspaceShellHeader` so the Agent workbench and Team shell no longer keep
  separate top-level header/menu structures.
- Extend canonical route parsing:
  - `/workspace`
  - `/workspace/teams`
  - `/workspace/teams/:team_id`
  - `/workspace/agents/:agent_id`
- Keep legacy `/teams` and `/` routes as compatibility aliases instead of forcing a hard redirect.
- Update Team shell navigation to use canonical workspace paths when opening teams and shell-level
  lenses.
- Add the first Team-shell global lens bar:
  - `Chat`
  - `Threads`
  - `Tasks`
  - `Members`
  - `Search`
- Keep the lens contract additive instead of rewriting Team-local tabs:
  - `Chat` / `Threads` -> Team conversation
  - `Tasks` -> Team kanban/tasks
  - `Members` -> Team overview
  - `Search` -> temporary shell placeholder while shared search rollup is still pending
- Start the left-rail vocabulary convergence by renaming the Team workflow section to `Channels`.
- Keep the Team detail sidebar on the same directory language as the selector shell by rendering a
  compact `Teams` section even outside selector mode; selector-only controls (search/create) remain
  hidden in detail mode.
- Promote standalone Agents one step closer to first-class workspace objects by routing agent
  selection through canonical `/workspace/agents/:agent_id` deep links.

## Validation

- `cd web && npm run test -- vite.config.test.ts src/workbench_header_menu.test.tsx src/app_route_selection.test.ts src/app.route_auth.test.ts src/pages/team/team_page_header.test.tsx`
- `cd web && npm run build`
- Chrome DevTools MCP:
  - `http://127.0.0.1:4173/workspace` now renders a shared `Workspace` header copy
  - `http://127.0.0.1:4173/workspace/teams` resolves as a valid Team selector alias
  - the workbench menu now exposes `Workspace / Teams / Settings / Logout`
  - local frontend-only smoke still shows the expected bootstrap JSON error because backend APIs
    were not running during the MCP check

## Follow-up

- Move the shared lens contract from Team-only mappings to true cross-entity workspace views.
- Promote Agent-local primary tabs (`Chat` / `Workspace` / `Profile` / `Activity`) inside the
  shared shell instead of relying on the legacy ACP workbench tab model.
- Converge the full shared rail so Team selector, Team detail, and Agent workspace all expose one
  compact `Channels / Teams / Agents` directory language.
