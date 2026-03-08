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

The local Bazel test path could not be completed in this environment because Bazelisk attempted to resolve/download `latest` and network access is restricted here. That local tooling limitation is separate from the CI failure root cause above.
