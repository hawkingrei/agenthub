# PR 126 CI Contract And E2E Alignment

## Context

PR `#126` failed in CI for two different reasons:

- Rust/Bazel still expected `POST /api/teams` to reject `spec.members = []` with `400`, but the product contract now allows creating an empty team first and binding agents later.
- Playwright E2E helpers/tests still assumed the older Team selector/detail flow and older input-dock sizing baselines.

## Changes

- removed the stale `members: []` case from `teams_api_rejects_invalid_spec` in `src/api/teams/tests_core.rs`
- updated `teams_router_http_contract` in `src/api/teams/tests_router.rs` to assert that empty-team creation succeeds and keeps `entrypoint`, `leader_member_id`, and `steps` absent until the first agent is added
- restored the missing `isSelected()` helper logic inside `web/tests/e2e/team_page.e2e.ts` so selector-driven team navigation no longer throws `ReferenceError`
- updated the Team E2E modal helper to target the current `Role model` field instead of the removed `Model override` field
- aligned Team E2E expectations with the current selector/detail behavior: after team creation the UI lands in the newly created team detail page instead of staying on the selector list
- aligned `web/tests/e2e/input_dock_layout.e2e.ts` touch-target thresholds with the intentionally denser input-dock sizing introduced by the recent mobile compaction pass
- relaxed `Add Agent` button assertions to avoid strict-mode failures when multiple visible entry points intentionally exist in the current Team detail layout
- updated the Team member-creation E2E helper to drive the Mantine `Role model` combobox through listbox options instead of using the removed native `<select>` path
- refreshed `tests/web_assets.rs` CSS snapshot expectations for the current compact input dock, jump-to-bottom button, history menu offset, mobile ACP header spacing, and ACP inline-code styling so Rust/Bazel asset checks match the current shipped stylesheet

## Validation

- `cargo test teams_api_rejects_invalid_spec`
- `cargo test teams_router_http_contract`
- `cd web && npm run lint -- tests/e2e/team_page.e2e.ts tests/e2e/input_dock_layout.e2e.ts`
- `cd web && PLAYWRIGHT_NO_WEBSERVER=1 PLAYWRIGHT_PORT=4175 npx playwright test tests/e2e/input_dock_layout.e2e.ts tests/e2e/team_page.e2e.ts --grep "touch-friendly|non-overlapping|team runtime controls update shared runtime badge|team create flow stores mission metadata before member setup|team setup keeps add agent wording after the first member binds"`
- `cd web && PLAYWRIGHT_NO_WEBSERVER=1 PLAYWRIGHT_PORT=4175 npx playwright test tests/e2e/team_page.e2e.ts -g "team member setup adds the first agent and appends more agents through spec updates"`
- `cargo test --test web_assets`
- `git -c core.fsmonitor=false diff --check`
