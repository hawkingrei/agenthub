# ACP Permission Tool Call Settlement

## Summary

PR #560 closes the ACP permission deny and timeout stall path by emitting a terminal failed
`tool_call_update` when a permission request does not select an allow option. It also narrows
prompt concurrency for Codex ACP sessions when the active prompt still has pending tool calls, and
removes stale mailbox false positives from `agenthub doctor agent-trace`.

## Background

A Team channel message was persisted and delivered to every member mailbox, but the member ACP pane
still appeared to stop returning output. Backend diagnostics showed the message delivery path was
healthy and the affected turns were waiting on permission requests that timed out to deny. The
runtime then needed a terminal tool-call event so the UI and diagnostics could clear the in-progress
tool call and accept later prompts safely.

## Scope

- `crates/agenthub-acp/src/lib.rs` settles denied or timed-out permission tool calls with a
  synthetic failed ACP `tool_call_update`.
- Prompt delivery still allows provider-supported concurrent Codex prompts, but queues a new prompt
  while the active prompt has pending tool calls.
- `src/diagnostics.rs` filters mailbox pending summaries so terminal Team runs do not make
  `agenthub doctor agent-trace` report old delivered work as a current stall.
- Web-only coverage tests were added for existing auth redirect and safe storage helpers so strict
  Codecov gates stay green after the backend fix.

## Key Decisions

- Deny and timeout outcomes must be terminal for the associated tool call. Relying on provider
  follow-up output is not sufficient because the failure path is already controlled by AgentHub.
- Codex ACP concurrent prompt support stays enabled in the normal case. The new queueing rule is a
  narrow safety gate only when the active prompt still owns unresolved tool-call state.
- `agent-trace` should count pending mailbox work for active Team runs and
  `shared_thread_mailbox`, but not for terminal run rows that already reached a finished state.
- This change does not close the broader Team ACP permission review routing backlog. Human-visible
  review cards, peer review routing, and no-self-review behavior remain separate validation work.

## Validation

```bash
cargo fmt --all --check
cargo test -p agenthub-acp
cargo test -p agenthub diagnostics::agent_trace
cargo build -p agenthub
cd web && npm exec vitest run src/auth_redirect.test.ts src/storage/safe_storage.test.ts
cd web && npm exec tsc -- --noEmit
gh pr checks 560 | cat
```

PR #560 CI was green after the final coverage push: Bazel Build, Bazel Test, Bazel Coverage, Rust
Fmt, Cargo Test, Proto Check, Web, Web E2E, Web E2E Mobile, Clippy, Distributed P2P Pipeline, and
Codecov project and patch checks all passed.

## Follow-Ups

- After merge and deployment restart, run a real Team ACP regression: send a message that triggers
  a permission request, let the request deny or timeout, and confirm the member tool call does not
  remain running.
- Continue the broader Team ACP permission review routing validation separately, including idle
  peer worker or coordinator fallback, no self-review, and human-visible review actions.
- If a future `agent-trace` verdict is `event_stream_present` while the UI remains stale, switch to
  the ACP rendering workflow and inspect SSE or frontend cache handoff instead of the mailbox path.
