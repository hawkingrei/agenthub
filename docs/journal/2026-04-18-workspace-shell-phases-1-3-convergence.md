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

## 2026-04-29 Follow-up

- extracted the Team-only `Search` placeholder card into a shared workspace-shell component:
  `WorkspaceLensPlaceholder`
- Team workbench search now uses shell-level language:
  - `Shared search is still being wired in`
  - `...the unified workspace search view is still a shell-level placeholder`
- this keeps phase-1 shell convergence moving toward shared lens behavior instead of leaving Team
  to maintain its own temporary search wording and chrome

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/components/workspace_lens_placeholder.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 2

- converged the standard workspace lens-bar item construction into one shared helper:
  `buildStandardWorkspaceLensItems(...)`
- root workspace and Team shell now reuse the same canonical lens set and the same `Search`
  placeholder hint instead of hand-maintaining two parallel arrays
- this keeps phase-1 shell reuse grounded in shared route/lens vocabulary instead of only sharing
  the outer header component

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/components/workspace_lens_items.test.ts`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 3

- extended the shared `WorkspaceLensPlaceholder` into the root `/workspace` shell so
  `?lens=search` no longer keeps a separate root-only fallback surface
- root workspace and Team shell now use the same shell-level search placeholder copy and card
  treatment instead of drifting on placeholder wording during phase-1 convergence
- this keeps the first shell rollout aligned around one temporary `Search` lens contract while
  shared search behavior is still pending

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/app.route_shell.test.tsx src/components/workspace_lens_placeholder.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 8

- converged the shell-level lazy panel loading chrome into one shared component:
  `WorkspacePanelLoadingFallback`
- root workspace workbench fallback and Team lazy panel fallback now use the same shell loading
  surface instead of keeping separate `Loading...` and `Loading panel...` treatments
- this keeps phase-1 shell fallback behavior aligned without touching the heavier Team bootstrap
  loading state

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/agents_route_shell.test.tsx src/pages/team/team_workbench_content.test.tsx src/components/workspace_shell_header.test.tsx src/components/workspace_panel_loading_fallback.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 9

- extended the shared workspace loading fallback into the remaining Team shell loading cards that
  were still hand-rendering one-off loading copy
- Team run-context loading and the lazy member ACP loading surface now reuse
  `WorkspacePanelLoadingFallback`, which keeps shell-level loading chrome aligned without touching
  the heavier Team bootstrap loading state

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team/team_workbench_content.test.tsx src/components/workspace_panel_loading_fallback.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 10

- converged the remaining Team bootstrap shell states onto the shared workspace shell primitives
- `TeamLoadingPanel` now renders through `WorkspacePanelLoadingFallback`, and
  `TeamUnavailablePanel` now renders through `WorkspaceLensPlaceholder` with a `Teams` lens
  identity instead of keeping a handwritten empty-state block
- this keeps Team bootstrap loading/unavailable chrome aligned with the rest of phase-1 shell
  fallback surfaces without changing the heavier bootstrap control flow

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team_workspace_state_panel.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 11

- converged the Team selector loading state onto the same shared workspace loading fallback surface
- the selector no longer shows a bare `Loading teams...` text row; it now renders
  `WorkspacePanelLoadingFallback` with Team-specific copy while keeping the filter controls hidden
- this keeps the shell-level team-selection loading chrome aligned with the rest of the phase-1
  workspace shell surfaces

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team/team_selector_panel.test.tsx src/pages/team_page.smoke.test.tsx -t "Loading teams"`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 12

- converged the root app route lazy fallback onto the shared workspace loading fallback surface
- `/join`, `/admin`, and `/teams` route-level lazy loading now render the same loading chrome as
  the rest of the workspace shell instead of a route-local plain text block
- this keeps root route loading behavior aligned with phase-1 shell fallback language without
  changing route selection or auth gating behavior

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/app.route_shell.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 5

- converged the `Search` lens hover/title hint onto the same shared shell copy surface as the
  placeholder card itself
- `buildStandardWorkspaceLensItems(...)` now reuses the exported shared search hint instead of
  keeping a second inline string, which removes another small phase-1 wording fork

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/components/workspace_lens_items.test.ts src/components/workspace_lens_placeholder.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 6

- aligned Team shell lens selection with the shared shell contract by resetting hidden Team-local
  tab state when operators switch back into shell-level lenses
- `channels` and `search` now restore the conversation baseline, `members` restores the overview
  baseline, and `tasks` restores the Kanban baseline instead of leaving stale member ACP/mailbox
  state parked behind the shell placeholder

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team/use_team_workspace_view_model.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 7

- converged the root `Machines unavailable` empty state onto the same shared workspace placeholder
  surface used by other shell-level non-content lenses
- root shell no longer hand-maintains a one-off empty-state card for the `Machines` lens, which
  keeps phase-1 shell fallback chrome more uniform across root workspace views

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/app.route_shell.test.tsx src/components/workspace_lens_placeholder.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 4

- converged the shell-level `Search` placeholder copy itself into one shared variant:
  `WorkspaceSearchLensPlaceholder`
- root `/workspace` and Team shell no longer each spell out the same temporary search copy by
  hand, which keeps the phase-1 placeholder contract from drifting as shell work continues

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/app.route_shell.test.tsx src/components/workspace_lens_placeholder.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`
