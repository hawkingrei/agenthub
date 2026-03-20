# 2026-03-19 Team UI Surface Refinement

## Goal

Continue the Team workspace visual cleanup so the primary surfaces read more like a lightweight
channel + task workbench and less like a stack of debug cards.

## Changes

### Channel timeline

- Reworked `Teams -> all` message list into a lighter timeline treatment:
  - stronger spacing rhythm between entries
  - narrower visual spine
  - softer agent cards and lighter human highlight
  - calmer composer surface
- Reduced the weight of the thread options row by replacing the full toolbar shell with a compact
  inline action row.

### Kanban surfaces

- Softened Kanban lane containers to use a lighter warm surface and larger radii.
- Reduced the visual heaviness of task cards while keeping active-vs-idle distinction.
- Changed task detail and run summary cards to flatter, calmer surfaces.
- Kept developer-only tools collapsed and moved them onto a dashed support surface so they stay
  visibly secondary.

### Rail + header compaction

- Continued shrinking the Team rail so it behaves more like a directory and less like a control
  panel:
  - removed extra second-line descriptions from the primary workbench entries
  - trimmed rail/header padding and button footprints
  - kept management actions out of the normal Team-workbench rail path
- Split the Team workspace header into thinner layers by reducing spacing and moving the
  `Add Agent / Start Team / Stop Team` controls into a compact mobile actions menu instead of
  forcing three large buttons into the top row on narrow screens.

### Mobile density

- Reduced Kanban lane/card/detail padding and minimum widths so the board feels less like a stack
  of oversized cards on smaller screens.
- Kept the same IA (`all / Kanban / Runs / Advanced`) instead of inventing a separate mobile-only
  flow, but tightened spacing so the primary content takes over more of the viewport.

## Validation

- baseline reference: inspected the logged-in Slock workbench via `agent-browser` and verified the
  thinner rail + `CHAT / TASKS` split direction before applying the local UI pass
- `cd web && npx vitest run src/pages/team_panels.test.tsx`
- `cd web && npm run lint -- src/pages/team_sidebar.tsx src/pages/team_page.tsx src/pages/team_task_panel.tsx src/pages/team_tasks_panel.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run build`
- `git -c core.fsmonitor=false diff --check`

## Notes

- This step intentionally stays in the visual layer only.
- It does not change Team task/run/channel semantics.
