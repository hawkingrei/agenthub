# Durable Loop Work Events

## Summary

Slice 7 connects canonical Team work to bounded loop intake and exposes authenticated member
activation and source recovery. The full transition remains incomplete until the later slices.

## Background

Offline configuration and provider execution do not by themselves make an offline worker receive
work. Message/task writes need durable scheduling intent, and a fresh session needs exact sources
even when post-commit mailbox fan-out is delayed or recent task history no longer includes them.

## Scope

- Addressed IM, canonical mentions, engaged-thread replies, assignment/reassignment/reopen/handoff.
- Actor `loop-context`, `loop-source`, and `loop-activate` commands with internal RPCs.
- Owner activation HTTP endpoint and authenticated scheduling provenance.
- Shared mention syntax and member reply-intake opt-out.

## Key Decisions

- Reuse the canonical SQLite transaction for both state and trigger acceptance. Existing message
  body outboxes retain their role; delivery copies do not create another work source.
- Budget rejection rolls back canonical writes. Suspension retains intake; disabling prevents
  automatic intake and rejects new explicit requests.
- Install scheduling context at authenticated request entrypoints. Never derive actor/activation
  authority from message payloads. Stable business keys preserve the original scheduling provenance
  on retries across activations.
- Page source references and resolve exact canonical messages under the live executor fence.
  Recovery does not rely on recent-message windows or a delivery copy completing first.
- The entry prompt change is a bounded recovery pointer (runtime tail), with a new recorded
  `loop-entry-v2` version. It grows only to name source recovery; no role skill or plugin changes.

## Validation

- `cargo build -p agenthub --bin agenthub --locked --offline` supplied the real actor CLI for the
  local fake-ACP fixture. The leader dispatched and exited, the worker recovered paginated sources
  and task state, then its report woke the offline leader in a new session without accepting the task.
- `cargo test -p agenthub --lib --locked --offline`: 864 tests passed, including 14 new focused work
  tests. Coverage includes canonical rollback, duplicate intake, file reopen before delivery,
  thread policy, stale assignment retirement, exact source recovery, signed identity, owner HTTP,
  capacity, and retry provenance across activations.
- The first full run had 859 passes and five local HTTP download failures caused by inherited proxy
  routing. Repeating with `NO_PROXY=127.0.0.1,localhost,::1` and the equivalent lowercase variable
  passed all 864. This is an invocation setting, not a repository or production configuration change.
- `cargo test -p agenthub-db loop_ --locked --offline`: 42 tests passed, including source pagination
  and compatibility with persisted optional user attribution.
- `cargo clippy -p agenthub -p agenthub-db --all-targets --locked --offline -- -D warnings`,
  `cargo fmt --all --check`, explicit rustfmt for the included HTTP test file, patch whitespace,
  generated-protocol equality, and changed documentation links passed.
- Provider/CLI fixtures used normal local socket permissions. Local Bazel and live paid-provider
  smoke were not run; the PR checks validate Bazel and coverage.

## Follow-Ups

Continue [the transition backlog](../todo.md#agent-loop-product-transition), including dependency and
standing triggers, shared MCP/Mem integration, role migration, observability/UI, apps/events, and Rara.
This checkpoint neither merges the PR stack nor deploys production behavior.
