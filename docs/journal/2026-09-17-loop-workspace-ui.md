# Loop Workspace UI

## Summary

Member profiles now expose offline configuration, explicit enable/suspend/resume, preflight,
assigned tasks, and durable activation history independently of process and manual-run state.
The [workspace UI contract](../features/agent-loop-workspace-ui.md) defines the stable behavior.
This is slice 13; app registration/event ingress and the remaining provider track are still open.

## Background

The previous member overview required an active execution run. The existing profile editor also
filled empty prompts with legacy role defaults and exposed the process idle watchdog. Saving a
loop member's description could therefore silently override its built-in loop role prompt.

## Scope

- Typed loop API clients, scoped configuration and activation controls, bounded history/detail pages.
- Existing member/channel profile routes, canonical task links, and an ACP diagnostics entry point.
- Mode-aware profile/forge/copy defaults and legacy watchdog isolation.
- A manual production-schema browser fixture reusing the real controller and fake ACP provider.
- Focused tests and this contract/evidence checkpoint; no production schema or protocol changes.

## Key Decisions

- Preserve process, policy, activation outcome, and task status as separate facts.
- Reconcile uncertain configuration writes through reads; preserve activation request identity
  across uncertain responses and page reloads instead of creating duplicate work.
- Keep a blank loop prompt blank through both draft conversion and page-level default effects.
  Prompt review classification: role boundary/default selection. Prompt template text and
  role-scoped skill entrypoints are unchanged; no prompt budget increase or new private tool pointer.
- Expose recorded wake conditions with explicit pagination limits, not an inferred global next wake.
- Pause history polling while inspecting details/older records and guard stale scope responses.

## Validation

Commands used for the focused and integration checks:

```bash
cd web
npm test
npm exec -- tsc --noEmit
npm run lint
npm run build
cd ..
cargo +1.96.0 test -p agenthub --lib loop_launch --locked --offline
cargo +1.96.0 clippy -p agenthub --all-targets --locked --offline -- -D warnings
cargo +1.96.0 fmt --all --check
git diff --check
```

The full web suite passed 1,563 tests before the final owner-roster compatibility case; the focused
member suite covers that follow-up. Thirteen Rust launch tests pass with four ignored fixtures,
and all-target Clippy passes with warnings denied. The web suite includes configuration conflicts, uncertain responses, request identity reuse,
member/authentication changes, zero-valued cursors, aborts, history inspection, read-only access,
preflight rejection, offline profiles, and task/process/policy separation. The Rust selection guards
the shared fake-provider fixture; the manual browser server is ignored in ordinary CI.

Chrome DevTools MCP 1.9.0 with isolated Chrome for Testing 149.0.7827.55 exercised real HTTP/SSE
routes and a file-backed production-schema test database:

1. Before: canonical member overview displayed `No Active Execution Run` with a stopped process.
2. Saved a member description while disabled/stopped. Readback retained the description and no
   configured prompt override. The legacy watchdog controls were absent.
3. Explicitly enabled execution, requested activation, and observed a normal provider exit with
   enabled policy and retained `no_actionable_work` history.
4. Suspended execution, sent a channel mention through the UI, and queued a manual activation.
   A task assigned to the worker was seeded through the authorized custom-channel task API;
   this task setup is not evidence of UI task creation. The UI showed the task as `open` and work
   as pending while suspended.
5. Resumed execution and held the fake provider at its prompt. Both process and activation showed
   running while the task remained open. Closed the application page, leaving only `about:blank`.
6. Released the provider with the application page closed. A read-only database observation
   confirmed generation 2 finished before reopening the application page.
7. Reopened member, task, and channel routes. Both finished activations, the original message, and
   the open assigned task remained visible without selecting a manual run. Activation detail
   showed trigger sources, verified cleanup, and successful tool observations.

Local artifacts use `/tmp/agenthub-loop-pr13-` with the suffixes
`before-canonical-member.{txt,png}`, `profile-final.txt`, `enabled.{txt,png}`,
`suspended-queued.txt`, `running-before-close.txt`, `reopened.{txt,png}`,
`history-details.txt`, `reopened-task.txt`, and `reopened-message.txt`.
The isolated fixture directory contains `page-closed.json` and
`completed-with-page-closed.json`. These are local test artifacts, not committed credentials.
No live model, production user data, or real external message recipient participated.

## Follow-Ups

- Complete slice 13 publication and applicable current-head CI; track it in [TODO](../todo.md).
- Continue the remaining app/provider slices independently of this UI validation milestone.
- Live provider and remote-executor parity remain outside this deterministic browser fixture.
