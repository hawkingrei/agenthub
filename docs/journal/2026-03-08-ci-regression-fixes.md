## Context

PR #116 still had two active CI failures after the earlier Teams/UI cleanup:

- `Bazel Build and Test`
- `Web E2E`

`Cargo` passed locally and in CI, which pointed to environment- or DOM-shape-sensitive regressions instead of core runtime breakage.

## Findings

### Bazel unit test

The failing test was:

- `api::teams::tests::start_agent_with_actor_context_injects_runtime_env_vars`

The implementation already injects:

- `AGENTHUB_ACTOR_RUN_ID`
- `AGENTHUB_ACTOR_ID`
- `AGENTHUB_ACTOR_CHANNEL`
- `AGENTHUB_ACTOR_CLI`

into the spawned process environment in `src/agent/manager.rs`.

The test previously inspected only the latest 500 persisted `env` output lines. That was stable under a smaller local environment, but brittle under Bazel/GitHub Actions where the spawned process inherits a larger sandboxed environment. In that environment, one expected variable line could fall outside the newest 500-event window even though the variable was injected correctly.

Fix:

- page through the full persisted event stream until the expected runtime variables are found or the poll loop times out

This keeps the runtime implementation unchanged and makes the assertion robust to larger sandbox environments.

### Teams Web E2E

Two concrete DOM regressions were identified from failed Playwright logs:

1. duplicate team-name headings in the Runs workspace caused strict-mode locator failures
2. selecting a team from the left rail auto-collapsed the team picker, which broke later sidebar-driven team switches in the same scenario

Fixes:

- demote the duplicate team-name title inside `TeamRunPanel` so the workspace header remains the single team-name heading
- keep the team picker list open after selecting a team, which matches the current rail-first interaction model and keeps `.teams-sidebar .team-item` available for subsequent switches

## Validation

Local checks executed:

- `cargo test start_agent_with_actor_context_injects_runtime_env_vars -- --nocapture`
- `cd web && npx vitest run src/pages/team_panels.test.tsx --pool=threads --maxWorkers=1`
- `cd web && PLAYWRIGHT_NO_WEBSERVER=1 npm run e2e -- tests/e2e/team_page.e2e.ts`
- `cd web && npm run lint`
- `make build-web`

The local Bazel test path could not be completed in this environment because Bazelisk attempted to resolve/download `latest` and network access is restricted here. That local tooling limitation is separate from the CI failure root cause above.

## Follow-up: Teams E2E selector migration

After the first CI fix, `Web E2E` still failed because the Playwright spec was still written for the older Teams navigation model.

The current product behavior is:

- primary no-team actions are rendered in the main workspace surface, not as unique global buttons
- `Debug` is no longer a top-level workspace control; it lives under `Advanced`
- `Mailbox` and `Member Console` are agent-focused advanced views, not global top-level tabs
- `Developer Mode` gates `Debug` and raw mailbox tools

Fixes:

- add E2E helpers that target the main workspace action buttons instead of ambiguous global locators
- add an E2E helper to open the current `Advanced` menu view path
- enable local `Developer Mode` in the tests that require `Debug` and `Mailbox Raw`
- update mailbox-flow tests to re-enter mailbox via `Runs -> Advanced -> Overview -> member row`
- restore the Team Forge final-stage primary button label to `Create Team` so the UI matches the modal intent and existing test expectations
- align unread/auto-follow assertions with the current mailbox lifecycle, where selecting a conversation may immediately mark it seen
