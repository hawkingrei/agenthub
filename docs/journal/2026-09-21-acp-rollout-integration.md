# ACP Rollout Integration

## Summary

Combine the original ACP slices 1-15 in a main-targeted integration branch. Native runtime slices
16-18 and their upstream prerequisites are deferred by the user. This checkpoint does not claim
production readiness before the remaining recovery and acceptance work is complete.

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
Other applicable recovery-head checks passed. Assembled-product checks and final-head CI remain
pending. No local Bazel command or build configuration change is part of this checkpoint.

## Follow-Ups

- Retain fencing for legacy ownership, missing evidence or a killed guardian; no operator assertion
  or database-delete shortcut is an accepted recovery path.
- Exercise scoped tools, events and retained browser history together.
- Update user/operator guidance to the actual supported behavior and finalize current-head CI.
