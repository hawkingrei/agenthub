# ACP Rollout Integration

## Summary

Combine the original ACP slices 1-15 in a main-targeted integration branch. Native runtime slices
16-18 and their upstream prerequisites are deferred by the user. This checkpoint qualifies the
explicitly enabled local Linux ACP path, including verified recovery and installed-runtime
acceptance. A scripted model proves runtime behavior, not autonomous task-solving quality.

## Background

The earlier implementation PRs were merged into dependency branches, so their merged status did
not put the implementation into main. The pre-native head of PR #1162,
`7c671973cf116fa900bb7df290ee37e225e14233`, preserves the complete ACP implementation. The current
remote slice-15 branch subsequently received native transport and is not a suitable ACP-only base.

## Scope

Durable activation lifecycle and scheduling, offline configuration, canonical task/message intake,
shared journaled MCP, scoped Mem, role prompts, history and UI, versioned Apps and signed events.
Local Linux, explicit opt-in and default fresh sessions remain the supported initial boundary.

## Key Decisions

- Merge main normally into the pre-native implementation; do not rewrite or replay published history.
- Preserve main's dependency updates and the implemented feature contracts and journal navigation.
- Keep one main-targeted review surface with focused follow-up commits and an explicit slice map.
- Retain uncertain execution ownership until cleanup is verified. New reservations record whether
  spawning was authorized; a delayed launcher must pass the same owner/generation transaction.
- The guardian inherits a private file lock and writes durable state before spawn and after descendant
  cleanup. Restart recovery acquires the lock and verifies its reservation identity before releasing
  ownership. No PID-based inference or operator assertion replaces this evidence.
- Use ACP for this rollout. Preserve the deferred draft and uncommitted upstream work independently.

## Validation

PR #1162's applicable checks passed at its historical head. Those results do not validate this new
integration head. The initial merge introduces four documentation resolutions and retains main's web
dependency updates. Follow-up commits add verified restart recovery and its focused regression cases.

The minimal inherited-lock proof confirmed that closing the launcher's file descriptor retains the
guardian's lock until exit. Focused store cases cover migration of old rows to `unknown`, delayed spawn
rejection, stale generation rejection, and preservation of recorded outcomes and canonical task state.
Validation commands for the recovery change:

```sh
cargo test -p agenthub-db loop_recovery -- --nocapture
cargo test -p agenthub --lib recovery -- --nocapture
cargo test -p agenthub --lib executor_guardian -- --nocapture
```

Integration head `f242ebc6` passed every applicable remote check, including Cargo, Clippy, Bazel build,
root/crate tests and coverage, browser/mobile, protocol generation, storage and documentation.
Recovery validation: 11 guardian cases, 110 root loop cases (6 opt-in fixtures ignored), 70 store loop
cases, and root/store all-target Clippy with warnings denied. The focused recovery selection also
covers a replacement manager with no old process handle and 9 passing cases. These selections overlap.
The first pass caught lazy event-directory creation; the implementation now creates that parent before
the witness. A parallel-fork test now waits for the transient CLOEXEC descriptor reference to close,
without weakening the exclusive-lock requirement.

Installed-runtime qualification found two concrete adapter failures. A fresh thread was configured
through `thread/resume` before any rollout existed. Settings now use `thread/settings/update` and
commit local state only after acceptance. Also, awaiting a complete prompt inside the ACP dispatch
handler prevented cancellation notifications from reaching a pending approval. Submission remains
ordered, while completion waits outside that handler.

The adapter's 155 unit cases cover the live-settings regression and existing cancellation state
transitions. The executable qualification below uses the built adapter and official Codex 0.150.1
with an isolated profile and local Responses server. It covers fresh configuration, native tools,
cross-process session loading/history, canceling a pending approval and ignoring its late response.
No paid model or real upstream account is used.

```sh
cargo test -p agenthub-codex-acp-runtime --lib
python3 scripts/verify_real_acp_runtime.py --adapter /path/to/agenthubd --codex /path/to/codex
cargo test -p agenthub --lib concurrent_terminal_status_update_and_handoff_do_not_both_apply
```

Recovery-head CI exposed one older concurrent-task fixture that omitted loop observer tables. It now
uses production migrations with the same multi-connection WAL pool and race assertion. A final
terminal update checks those observers whichever contender wins. The focused regression passes.
Other applicable recovery-head checks passed. No local Bazel command or build configuration change
is part of this checkpoint.

Adapter-fix head `915f7ea4` passed every applicable remote check, including both Bazel and Rust
coverage. The final acceptance/documentation commit requires its own current-head checks before
merge; the PR check suite is authoritative for that result.

### Assembled acceptance

The opt-in `loop_real_acp_dispatch_worker_and_fresh_acceptance` fixture uses production migrations,
the real adapter, official Codex 0.150.1, native command tools, signed actor RPC, and real MCP shims.
Only the Responses model and external Mem/App services are scripted. It proves:

- signed event intake and duplicate receipt identity;
- coordinator dispatch, offline worker evidence, and acceptance from a fresh coordinator session;
- one entry per activation with multiple native rounds and the existing role skill pointer;
- current scoped Context Lens recovery on each fresh activation;
- native deferred-tool discovery and namespaced MCP calls;
- explicit foreign-space rejection and omission-based binding to the correct Mem space;
- pinned App version, one actual upstream write, and rejection after binding revocation;
- isolation of Mem, App and event credentials from the provider;
- a fourth activation retaining independent local progress during Mem outage;
- canonical progress receipts linking each new task note to its activation outcome;
- release of execution reservations after each completed activation.

Reproduce with a built `agenthub` control CLI and matching `agenthubd` adapter:

```sh
TEST_MEM_UPSTREAM_KEY=acceptance-mem-key \
TEST_APP_TOKEN=acceptance-app-key \
TEST_EVENT_KEY=ExMTExMTExMTExMTExMTExMTExMTExMTExMTExMTExM= \
CARGO_BIN_EXE_agenthub=/absolute/path/to/agenthub \
LOOP_REAL_ACP_BINARY=/absolute/path/to/agenthubd \
LOOP_REAL_CODEX_BINARY=/absolute/path/to/codex \
cargo test -p agenthub --lib loop_real_acp_dispatch_worker_and_fresh_acceptance -- --ignored --nocapture
```

For browser acceptance, also set `LOOP_REAL_BROWSER_DIR` to a new directory and `LOOP_UI_WEB_DIR`
to the absolute built web directory. `ready.json` contains an isolated fixture login and URL. Open
the planner's member profile, close the page while its first activation is held, then remove
`browser-hold` from the provider directory. `completed` signals the four verified activations.
The optional harness then runs the actual scheduler for suspension/admission inspection until the
operator creates `stop`. Suspend execution, activate the member, verify the retained pending entry,
then resume execution and inspect the new finished entry. All services and profiles are isolated
from user configuration.

Chrome DevTools MCP observed RUNNING before page closure, then EXITED and three FINISHED coordinator
history entries on reopening. The fourth activation's details retained `mem context unavailable`;
Kanban retained the completed task, worker result, and coordinator decision. Database inspection
confirmed four finished activations and zero retained reservations. Browser snapshots/screenshots
are local artifacts under `/tmp/agenthub-real-browser-20260921/`.

The final browser pass also suspended admission, accepted an operator trigger, and retained it
pending across scheduler ticks without acquiring a reservation. Resuming through the UI let the
actual scheduler execute that activation. Final inspection found five finished activations, five
canonical progress receipts, zero reservations, zero consecutive no-progress counts, and the
completed task. The first fixture attempt omitted `task_note_id`; the resulting `no_progress_limit`
correctly blocked admission. The fixture now links actual dispatch, worker result, acceptance and
local-work notes instead of weakening the budget. Final snapshots, screenshots and `verified.json`
are under `/tmp/agenthub-real-browser-evidence-20260921/`.

The real daemon-crash regression also passes at all four boundaries: before sending, after sending
without a response, after receiving a response before its durable commit, and after durable success.
A replacement daemon acquires independent ownership and preserves ambiguous writes as unknown;
the real MCP shim cannot replay them. Reproduce with
`cargo test -p agenthub --lib real_mcp_proxy_survives_daemon_crashes_without_replaying_an_unknown_write`.

Prompt review classification: skill/recovery-pointer regression coverage. Prompt text and skill
entrypoints are unchanged; the fixture checks delivery of the existing `team-loop-runtime` pointer.
The operator guide documents the local Linux/provider matrix and enable, inspect, suspend/resume,
Mem/App setup, and verified restart recovery. User docs and the current web build pass.
Final local validation also passes 110 root loop cases (7 opt-in cases ignored), with the real ACP
fixture executed separately, plus root/adapter all-target Clippy with warnings denied.

## Follow-Ups

- Retain fencing for legacy ownership, missing evidence or a killed guardian; no operator assertion
  or database-delete shortcut is an accepted recovery path.
- Merge PR #1169 after review and applicable current-head CI.
