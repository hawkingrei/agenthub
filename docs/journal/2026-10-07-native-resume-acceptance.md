# Native Resume Acceptance

## Summary

Admit local loop Resume only when the runtime supports durable approvals and the full
resume/query/resolve/reentry control set. After explicit uncertainty reconciliation,
finish the activation as waiting for new durable input and retire its executor.

## Background

Actual-process acceptance exposed a lifecycle gap: resolution moved the provider to
Idle but left the outer activation running. Direct loop input correctly remained
forbidden, and its reservation prevented new durable work from entering. A private
runtime-handle test had bypassed the public intake constraint and missed this gap.

## Scope

- Capability negotiation before any event-owner binding or conversation opening.
- Fenced recovery completion after a matching durable ACK/event prefix.
- Clean restart, restored approval and uncertain approved-effect process scenarios.
- Opt-in browser review against production schema, HTTP/SSE and the actual native binary.
- No schema, wire, task-completion or standalone continuity change.

## Key Decisions

Require complete recovery support even when the last observed phase was Idle. The
checkpoint may have advanced afterward. Missing support cannot fall back to fresh.
Current sources are registered under each new activation; old transcript context
does not reinstall old credentials or source authority.

Only confirmed reconciliation records `waiting`/`input`, without a continuation or
business-task completion. The daemon owns this operation through caller disconnect.
Supervised cleanup releases the reservation, and a later explicit durable trigger
creates a new activation using the preserved provider conversation. Standalone
reconciliation continues to allow a later explicit input on its live owner.

Contract: [direct runtime integration](../features/rara-direct-integration.md).

## Validation

```bash
cargo test -p agenthub-rara --lib
cargo test -p agenthub --lib agent::manager::loop_launch::tests::native::
AGENTHUB_RARA_TEST_BINARY=/absolute/path/to/pinned-runtime cargo test -p agenthub --lib native_resume_process_ -- --ignored
cargo clippy -p agenthub -p agenthub-rara --lib --tests -- -D warnings
cargo fmt --all --check
```

Actual-process qualification uses producer revision
`178dfecf6599ad536f53f5f207d37d547b510a6b`, binary SHA-256
`70c42d4622ded1c04103a0d7a2087dbe06083930d0e0646bb80c6f28f8a32dac`.
The model endpoint is a deterministic local fixture, not configured-provider evidence.
All three restart scenarios pass: retained history with current activation context;
kill while awaiting approval with stale callback rejection and one effect; kill during
an approved effect, explicit reconciliation without replay, then new durable work.

The browser fixture uses `LOOP_NATIVE_BROWSER_DIR` and `LOOP_UI_WEB_DIR` with the
uncertain-effect case. Run `native_recovery_process.e2e.ts` against its `ready.json`
manifest using the same directory and `PLAYWRIGHT_NO_WEBSERVER=1`. The browser records
the exact recovery POST, sends no input, and checks retained conversation events after
the Rust fixture completes a subsequent durable activation. It opens member history
after reload to verify the persisted waiting outcome. Process diagnostics select a live
owner and do not discover the latest stopped loop transcript on a cold page load.
Normal CI skips this
opt-in browser case when the fixture directory is absent.

Results: 58 protocol cases pass (one existing opt-in case ignored); 18 managed native
cases pass, including stale owners, missing capabilities, delayed/mismatched receipt
prefixes and caller disconnect. The three process scenarios and the real-backend
browser case pass. The 15 focused member-panel/history tests, TypeScript, ESLint,
production web build, warnings-denied Rust Clippy, formatting and 123 local
documentation links pass. Chrome DevTools MCP was unavailable; browser evidence uses
the repository Playwright harness and its matching cached Chromium build.

## Follow-Ups

Complete standalone continuity, installed/configured-provider qualification and
upstream publication under its pending destination-specific authorization. These
remain active in [the transition TODO](../todo.md). Deterministic process/browser
acceptance does not close those milestones.
