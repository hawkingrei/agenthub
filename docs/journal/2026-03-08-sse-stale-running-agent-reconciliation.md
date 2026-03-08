# SSE Stale Running Agent Reconciliation

## Background

After abrupt `agenthub` termination, especially direct `Ctrl+C`, users could observe a stable mismatch:

- SQLite still reported an agent/session as `running`
- the frontend subscribed to `/sse/agents?...` for that agent
- backend SSE returned `404 agent not running`
- the browser kept retrying because API state still advertised the agent as running

This means runtime truth and persisted lifecycle state diverged. The in-memory runtime handle was gone with the process, but DB rows were not always converged to `exited`.

## Scope

- Reconcile stale `running` agent/session rows when SSE or send-input discovers runtime absence.
- Add best-effort shutdown cleanup for direct signal-driven process exit.
- Add focused regression tests for the runtime-reconciliation path.

Out of scope:

- multi-process/shared-DB runtime ownership
- process reattach to already-running external agent subprocesses

## Key Decisions

1. Add a reusable manager helper:
   - `AgentManager::reconcile_runtime_absence(agent_id)`
   - check whether DB still says `running`
   - if no in-memory runtime handle exists and the agent is not in the startup window, mark it stale
2. Reuse one reconciliation implementation for all stale-runtime paths instead of duplicating inline SQL:
   - `send_input`
   - SSE single-agent subscription
   - SSE multi-agent subscription
3. Extend transport self-healing:
   - when SSE subscription fails because runtime output is unavailable, reconcile DB state before returning `404`
   - this lets the next `/api/agents` refresh stop advertising the agent as `running`, so browser retry loops converge
4. Add best-effort graceful shutdown cleanup:
   - listen for `Ctrl+C` and `SIGTERM`
   - before server shutdown completes, mark startup-visible `running` agents as `exited`

## Files Changed

- `Cargo.toml`
- `src/app.rs`
- `src/sse.rs`
- `src/agent/manager.rs`
- `src/agent/manager/runtime.rs`

## Validation

Executed during development:

- `env CARGO_TARGET_DIR=/tmp/agenthub-cargo-stable-test RUSTC=$HOME/.rustup/toolchains/1.93.1-aarch64-apple-darwin/bin/rustc RUSTDOC=$HOME/.rustup/toolchains/1.93.1-aarch64-apple-darwin/bin/rustdoc $HOME/.rustup/toolchains/1.93.1-aarch64-apple-darwin/bin/cargo test stale_running_agent`

Covered regressions:

- stale runtime absence marks agent/session `exited`
- SSE multi-agent route reconciles stale `running` rows before returning `404`

## Risks And Follow-up

- Shutdown cleanup remains best-effort; force-kill paths that bypass signal handling can still rely on next-request reconciliation.
- The reconciliation model assumes single-process runtime ownership and local in-memory truth.
- End-to-end browser verification should still confirm the SSE retry badge clears after abrupt restart/kill and subsequent API refresh.
