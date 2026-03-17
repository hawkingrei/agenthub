# PR 126 Review Fix Pack

## Context

PR `#126` still had several focused review comments worth fixing inside the same change set:

- the workbench connection badge had collapsed to a monochrome dot even though the component still carried tone metadata
- ACP tab buttons still relied on the default `<button>` submit behavior
- the Team `Delete Team` action had lost a clear destructive surface
- `CreateAgentModal` had picked up Team-specific styling even when rendered from the standalone Agents page
- Team E2E mocks no longer enforced the backend `expected_updated_at` optimistic-concurrency contract
- one Rust router test still depended on list ordering
- ACP plan bubble background had diverged between Tailwind utility presets and the legacy compatibility stylesheet

## Changes

- restored tone-aware root classes in `web/src/components/workbench_connection_badge.tsx` and tightened unit coverage in `web/src/workbench_connection_badge.test.tsx`
- added explicit `type="button"` to ACP panel tab/action buttons in `web/src/components/acp_panel.tsx`
- made Team `Delete Team` use a single destructive button surface in `web/src/pages/team_run_panel.tsx`
- scoped Team-specific modal cosmetics behind `teamStyled` in `web/src/components/create_agent_modal.tsx`, and enabled that flag only from the Team flow in `web/src/pages/team_page.tsx`
- hardened `web/tests/e2e/team_page.e2e.ts` so Team spec updates must send `expected_updated_at` and stale updates return `409`, matching the real backend contract
- removed the unused `role` parameter from the Team E2E member-creation helper to keep the helper signature honest
- made `src/api/teams/tests_router.rs` order-independent when asserting listed Team ids
- aligned `ACP_BUBBLE_PLAN_CLASS` with the shipped compatibility background in `web/src/ui/tailwind_classes.ts`

## Deferred Follow-Up

- The `createAgent` then `updateTeamSpec` orphan-agent window is real, but it is not a clean patch-level fix inside PR `#126`.
- A proper fix should introduce a backend Team-member create/bind API so agent creation and Team membership update share one transaction boundary.

## Validation

- `cd web && npx vitest run src/workbench_connection_badge.test.tsx src/acp_panel.test.tsx src/create_agent_modal.test.tsx src/create_agent_modal.interaction.test.tsx`
- `cargo test teams_router_http_contract`
- `cd web && npm run lint -- src/components/workbench_connection_badge.tsx src/workbench_connection_badge.test.tsx src/components/acp_panel.tsx src/pages/team_run_panel.tsx src/components/create_agent_modal.tsx src/pages/team_page.tsx src/ui/tailwind_classes.ts tests/e2e/team_page.e2e.ts`
- `cd web && npm run build`
- Chrome DevTools MCP baseline: inspected the deployed `https://agenthub.hawkingrei.com/` surfaces to confirm the current badge/button/modal baseline before the local fixes; deployed regression remains pending until this change ships
