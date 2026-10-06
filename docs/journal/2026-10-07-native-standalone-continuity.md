# Standalone Native Continuity

## Summary

Local Linux standalone native launches now retain their own durable execution owner and
conversation binding. Explicit resume preserves provider history through process and manager
restart, restores approvals under a new local owner, and keeps uncertain effects blocked until
review. The default standalone policy remains fresh.

## Background

Loop continuity already used durable reservations and guardian cleanup evidence. Standalone
launches always created conversations and lacked an equivalent ownership fence. Reusing a closed
receipt or an ended local session as cleanup proof could admit a replacement beside old descendants.
The ACP failed-resume fallback also cannot represent an uncertain native opening safely.

## Scope

- Add standalone owner/conversation tables through the existing control-store migration.
- Extend guardian witnesses with a separate identity namespace while preserving existing loop files.
- Integrate reservation, guarded spawn and exact cleanup into failure, stop, transport loss,
  natural exit and daemon shutdown paths.
- Add `rara.standalone_session_policy`, complete resume negotiation, current-owner input checks
  and an authorized stopped-session continuity reset through the existing clear endpoint.

## Key Decisions

- Standalone launches have actor/local launch/daemon/generation identity and never synthesize
  Team membership or loop credentials. Loop and standalone admission exclude each other.
- Reserved owners retire through a conditional state transition. Guarded owners require an exact
  exclusive guardian witness kept alive through the retirement commit; status or time is insufficient.
- Retired owners preserve generation and cleanup history. Reset forgets only conversation identity;
  explicit actor deletion keeps existing deletion semantics after the quiescence guard.
- Resume verifies effective workspace/configuration and records opening intent before sending.
  Only the exact retired launch's closed accepted receipt repairs a missing binding. Unknown results
  remain blocked until explicit fresh/reset; no retry or fresh fallback occurs automatically.
- Restored callbacks, input and recovery controls verify the new standalone owner. Review does
  not replay an approval or tool, send a prompt, or create Team work. Non-Linux fresh compatibility
  remains available; standalone resume requires the Linux guardian.

## Validation

The focused SQLite selection passes 11 cases, including reopen, concurrent reservation,
spawn/cleanup competition, stale identity, mode arbitration, opening repair, reset and deletion.
The actual native process selection passes three cases against the qualified local producer and a
deterministic local model: clean manager/database restart with retained history, kill during pending
approval with a new callback, and kill during an approved append with no replay after review.
This is local candidate qualification, not configured-provider or upstream-release evidence.

The native manager selection passes 41 cases, including caller disconnect before spawn settles.
Existing selections pass 18 native-loop, 13 guardian, 14 scope/configuration and 11 session cases;
the loop store passes 78 cases and configuration passes 3. Root and DB Clippy with warnings denied,
formatting, whitespace and 126 local documentation links pass. The Linux timeout fixture now checks
reaped parent/descendant PIDs and retired ownership, matching the guardian's SIGKILL cleanup instead
of expecting a provider SIGTERM handler.
The two reset/history API cases pass, including capability denial, held-owner conflict, provider
selection and preservation of retired generations after reset.

The opt-in standalone browser case passes against real API/SSE routes, a reopened database/new
manager and the actual candidate process. Recovery confirmation sends no input and the backend
observes no additional model request before explicit input. One subsequent instruction preserves
the earlier provider context, produces one response and leaves exactly one original append.
Refreshing retains the response and cleared recovery state. Before/after screenshots were inspected;
no browser page errors occurred. The shared browser-server regression also passes the existing Team
recovery case. TypeScript, ESLint and the production web build pass. Chrome DevTools MCP was
unavailable, so these checks use the repository's Playwright fallback.

The shared server writes its private authentication manifest atomically and stops on drop. Both
browser fixtures remain opt-in; ordinary process tests still exercise direct API controls without
requiring a browser. Browser artifacts are under `target/loop-review-validation/standalone-browser-*`
and `target/loop-review-validation/loop-browser-shared-*`; manifests contain local credentials and
must not be published.

Commands for the completed component checks and final regression gates:

```bash
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/loop-review-validation cargo test -p agenthub-db native_sessions --lib
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/loop-review-validation cargo test -p agenthub-config rara --lib
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/loop-review-validation cargo test -p agenthub --lib agent::manager::rara::tests
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/loop-review-validation cargo test -p agenthub --lib standalone_native_process -- --ignored --test-threads=1
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/loop-review-validation cargo clippy -p agenthub --lib --tests -- -D warnings
cargo fmt --all --check
git diff --check
```

Actual-process checks require `AGENTHUB_RARA_TEST_BINARY` and a guardian-capable local binary.
For browser acceptance, build `web/dist`, set `LOOP_UI_WEB_DIR` to its absolute path and set
`STANDALONE_NATIVE_BROWSER_DIR` to a fresh private directory. Start the ignored
`standalone_native_process_reconciles_without_replaying_uncertain_effects` case and, once its
`ready.json` appears, run from `web/`:

```bash
PLAYWRIGHT_NO_WEBSERVER=1 PLAYWRIGHT_MINIMAL_RUNTIME=1 npm exec playwright -- test tests/e2e/native_standalone_recovery_process.e2e.ts --project=chromium --workers=1
```

The Playwright process must inherit the same `STANDALONE_NATIVE_BROWSER_DIR`.
Remote exact-head checks remain the publication gate recorded with the PR.

## Follow-Ups

- Complete installed/configured-provider qualification and upstream publication after the pending
  destination authorization; do not substitute deterministic local model evidence for those gates.
- Track exact-head CI and retained activation-to-transcript navigation in [TODO](../todo.md).

Contract: [direct runtime integration](../features/rara-direct-integration.md).
