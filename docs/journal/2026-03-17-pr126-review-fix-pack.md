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
- replaced the new input-dock `!important` overrides in `web/src/styles.css` with more specific `.input.docked ...` selectors so the compact interrupt/history/history-item sizing still wins without adding new `!important` debt
- hydrated `TeamRunRecord.summary` in `TeamManager::list_runs`, so `GET /api/teams/:id/runs` now matches the active-run endpoints and surfaces fallback summaries consistently
- switched the Team ACP input dock send guard in `web/src/pages/team_member_acp_panel.tsx` from async state-only gating to a synchronous ref mutex, and disabled the send action while a prompt is in flight to avoid duplicate rapid sends
- updated the stale Team E2E navigation labels from `Mailbox` / `Tasks` to the current `all` / `Kanban` workbench IA in `web/tests/e2e/team_page.e2e.ts`
- corrected the desktop metadata-overlap E2E to leave the agent-focused workspace first (`sidebar all`), then open the Team-level `Mailbox` tab; the failure was caused by looking for Team mailbox controls while still inside agent workspace, not by the mailbox label itself
- made the task-mailbox forwarding core test assert against the task auto-created run instead of racing a same-second manually created run in `src/api/teams/tests_core.rs`
- changed the orchestrator start-step error test to break `team_run_events` rather than dropping `team_steps`, so `dispatch_step()` can still hydrate the run before exercising the intended `start_step` failure path

## Deferred Follow-Up

- The `createAgent` then `updateTeamSpec` orphan-agent window is real, but it is not a clean patch-level fix inside PR `#126`.
- A proper fix should introduce a backend Team-member create/bind API so agent creation and Team membership update share one transaction boundary.

## Validation

- `cd web && npx vitest run src/workbench_connection_badge.test.tsx src/acp_panel.test.tsx src/create_agent_modal.test.tsx src/create_agent_modal.interaction.test.tsx`
- `cargo test teams_router_http_contract`
- `cd web && npm run lint -- src/components/workbench_connection_badge.tsx src/workbench_connection_badge.test.tsx src/components/acp_panel.tsx src/pages/team_run_panel.tsx src/components/create_agent_modal.tsx src/pages/team_page.tsx src/ui/tailwind_classes.ts tests/e2e/team_page.e2e.ts`
- `cd web && npm run build`
- `cargo test team_task_messages_api_forwards_human_chat_to_active_run_mailbox`
- `cargo test dispatch_step_returns_error_when_start_step_returns_error`
- `cargo test list_runs_supports_status_filter_and_cursor`
- `cd web && npx vitest run src/pages/team_panels.test.tsx`
- targeted Playwright re-run still depends on the local dev server bootstrap path; if sandboxed execution times out, rerun outside the sandbox and record the result in the PR thread
- Chrome DevTools MCP baseline: inspected the deployed `https://agenthub.hawkingrei.com/` surfaces to confirm the current badge/button/modal baseline before the local fixes; deployed regression remains pending until this change ships
