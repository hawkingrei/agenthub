# Team Empty-State And Agent Modal Reference Alignment

## Context

The Team workbench empty-state and the `Add Agent` modal still carried a heavier multi-card/brutalist treatment than the reference workbench we wanted to learn from. The reference page pattern worth borrowing was not the color palette, but the layout grammar:

- a lighter single-layer header
- compact uppercase tabs and labels
- inline info strips for key facts instead of stacked checklist cards
- tighter modal sections with label/value summaries before the long form fields

## Changes

- softened the local Team workbench shell treatment in `web/src/pages/team_page.tsx` by moving key panel/button/badge styles from thick black borders toward the existing product border tokens and shallow shadows
- replaced the empty-team setup checklist card with a two-row info strip (`Goal / First Agent / Unlocks` and `Step 1 / Step 2 / Step 3`) so the empty-state reads like a workbench summary instead of a wizard card
- demoted the team goal block under the workspace title into a compact labeled text strip
- refreshed `web/src/components/create_agent_modal.tsx` so the modal header, action buttons, and section cards use the same lighter shell language
- added a compact runtime/config strip inside the modal (`Preset / Command / Mode`) plus a second role-oriented strip (`Role / Skills / Prompt`) ahead of the editable fields
- made role selection explicit in the `Add Agent` modal with a compact `Leader / Worker` segmented control, while keeping the Team constraints visible in-product (`first agent must be leader`, `one leader per team`)
- tightened the role-profile copy so the modal no longer repeats `Role` in multiple stacked sections; the lower strip now reads as `Focus / Skills / Prompt Scope`, with role-specific copy resolved from a single helper
- hardened the worktree default helper chain against `undefined` / `null` inputs so stale defaults payloads or missing modal roots no longer crash on `.trim()` during `Add Agent`
- removed the duplicated visible `Code mode` label in the modal mode row and left a single `Mode = Code|Chat` value with an unlabeled switch
- followed up with a broader shell pass across `web/src/pages/team_page.tsx` and `web/src/pages/team_sidebar.tsx` to pull the workbench cards, tab rows, operation chips, runtime pill, and sidebar sections back to the lighter product border/shadow system instead of the earlier thick black outline treatment
- extracted the role/default-name/workdir calculations into `web/src/pages/team/forge_helpers.ts` so modal role switching reuses a single pure path instead of ad-hoc inline branching
- updated `web/src/create_agent_modal.test.tsx` plus new `web/src/pages/team/forge_helpers.test.ts` coverage to pin the summary strip and role/default calculations

## Validation

- `cd web && npx vitest run src/create_agent_modal.test.tsx src/create_agent_modal.interaction.test.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run lint -- src/components/create_agent_modal.tsx src/create_agent_modal.test.tsx src/create_agent_modal.interaction.test.tsx src/pages/team_page.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run build`
- `git -c core.fsmonitor=false diff --check`

## Chrome MCP Notes

Baseline:

- inspected the deployed page at `https://agenthub.hawkingrei.com/teams/276a2682-9ce7-4af5-aa6c-f12575d13c37`
- the deployed version still showed the older heavier empty-state and modal wording/layout

Reference study:

- inspected `https://app.slock.ai/agent/203a67aa-b73f-4d38-afa1-1fc07a11216d?tab=info`
- the useful pattern was a compact `title + tabs + info strip` layout, not the black/yellow palette

Post-edit local regression:

- rebuilt `web/dist`
- opened `http://127.0.0.1:4175/teams/team-empty` under a Chrome DevTools MCP page with injected local API mocks
- confirmed the empty-state now renders a compact `Team Setup` summary strip, the `Add Agent` modal renders the new `Preset / Command / Mode` plus `Role / Skills / Prompt` strips, and the main workspace/sidebar shell no longer rely on the old thick black outline blocks
