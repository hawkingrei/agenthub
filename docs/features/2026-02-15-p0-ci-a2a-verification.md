# P0 CI and A2A Verification Closure

## Summary

Close the P0 verification backlog for CI/Codecov, Playwright execution, A2A
actor-runtime wiring, and worktree default-root validation.

## Scope

- `docs/todo.md`
- `docs/features/2026-02-14-a2a-team-actor-runtime-context-start-api.md`
- `docs/features/2026-02-15-a2a-team-review-followup-orchestrator-relay-hardening.md`
- `src/api/teams/tests.rs`

## Evidence

### CI workflow split and execution

Verified workflow split (`Rust` / `Web` / `Web E2E`) via recent successful
runs on both `push main` and `pull_request`:

- Rust: `22030271235` (push), `22030798540` (pull_request)
- Web: `22030271231` (push), `22030798537` (pull_request)
- Web E2E: `22030271226` (push), `22030798535` (pull_request)

### Codecov artifact and flag verification

Codecov API confirms both coverage uploads exist and are processed for main
commit `1acef585f2257057f20388b87c3397f543941247`:

- `web-coverage` with flag `web` (`processed`)
- `rust-coverage` with flag `rust` (`processed`)

Patch diff coverage is available in Codecov commit report:

- `report.totals.diff[5] = 91.80503`

### Local Playwright verification

Executed locally against running Vite dev server:

```bash
cd web
PLAYWRIGHT_NO_WEBSERVER=1 npm run e2e -- --project=chromium
```

Result: `3 passed`.

### A2A actor-runtime verification

Added and executed runtime env injection test:

```bash
cargo test start_agent_with_actor_context_injects_runtime_env_vars -- --nocapture
```

This verifies subprocess env wiring for:

- `AGENTHUB_ACTOR_RUN_ID`
- `AGENTHUB_ACTOR_ID`
- `AGENTHUB_ACTOR_CHANNEL`
- `AGENTHUB_ACTOR_CLI`

Also re-validated orchestrator actor-context dispatch flow:

```bash
cargo test dispatch_once_injects_actor_runtime_and_supports_inbox_ack_flow -- --nocapture
```

### Replay-protection guidance

Added receiver-side anti-replay guidance with timestamp skew and idempotency
dedupe reference flow in:

- `docs/features/2026-02-15-a2a-team-review-followup-orchestrator-relay-hardening.md`

## Follow-ups

- If desired, add a lightweight script that snapshots Codecov upload metadata
  for the latest `main` commit and stores it under CI artifacts for audit.
