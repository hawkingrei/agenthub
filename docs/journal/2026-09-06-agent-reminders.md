# Agent Reminder Implementation Checkpoint

## Summary

Standalone agents and Team members now share self-service reminder commands with durable scheduling,
fenced dispatch, retry backoff, scope provenance, and explicit submission receipts. The canonical
contract is [Agent Reminders](../features/agent-reminders.md).

## Background

The existing `agent_time_triggers` table and Team CLI supported one-shot follow-ups. Standalone
processes lacked a default self identity and capability instruction. Completion and retry updates
could overwrite cancellation, startup reset every in-flight dispatch, and immediate retry allowed
failing agents to repeatedly occupy the due batch. `fired` did not distinguish queue submission from
agent execution.

## Scope

- Preserve one-shot public endpoints and the existing persisted status vocabulary.
- Add standalone runtime identity and a bounded ACP instruction without Team role skills.
- Capture source session/Team/run plus an optional task or message reference.
- Use atomic claims, expiring leases, fenced completion, bounded concurrent dispatch, and backoff.
- Queue ACP reminders behind active turns and preserve ordinary input steering.
- Add an explicit remote reminder RPC, source snapshots, and visible retry/submission information.

## Key Decisions

- Relative deadlines are computed in the manager after source lookup using the same timestamp as
  record creation. Legacy absolute deadlines remain accepted by the RPC.
- `fired` remains wire compatible but means submitted. The web inspector labels it `submitted`.
- Cancellation fences later database updates but cannot retract submission already in progress.
- Session restarts are compatible; new reminders cannot cross Team/run scope changes. Legacy rows
  retain their existing behavior without invented provenance.
- Eight concurrent attempts, a batch cap of 32, a ten-second submission timeout, and 90-second
  leases bound the normal batch. Failed submissions use exponential backoff capped at five minutes.
- Remote delivery uses a distinct RPC, so older peers reject it rather than silently dropping idle
  semantics. Missing/unbound remote source snapshots also fail closed.
- Stopped processes are not restarted. Reminder-only Worker tokens cannot manage other agents or
  access Team tasks. Local credential renewal continues through the existing trusted-host connector.

## Validation

Passed locally:

- `cargo check --locked -p agenthub`.
- `cargo build --locked -p agenthub --bin agenthub` for the test fixture's real-binary prerequisite.
- `cargo test --locked -p agenthub --lib reminder`: cancellation before/during dispatch, concurrent
  claims, lease recovery, stale-attempt fencing, backoff fairness, bounded timeout, stopped-agent
  behavior, self identity, source/run boundaries, and reminder-only authorization.
- `cargo test --locked -p agenthub --lib time_trigger`: existing CLI, HTTP, and internal RPC
  create/list/cancel compatibility, including legacy absolute deadlines.
- `cargo test --locked -p agenthub --lib reminder_relative_deadline`: one-second relative deadlines
  use the persisted creation timestamp.
- `cargo test --locked -p agenthub-db -p agenthub-acp reminder`: legacy migration/readback,
  repeatability, deferred ACP policy, and bounded provider-neutral instructions.
- `cargo test --locked -p agenthub --lib remote_agent_grpc_control_starts_inputs_and_lists_events_over_tls`:
  actual mTLS source snapshot and reminder submission, with output readback from a raw stdin test
  process alongside ordinary input. This does not prove a live LLM provider turn.
- Focused web component tests cover submission wording, retry provenance, serial refresh, and
  discarding responses from a previous agent; TypeScript, ESLint, and production Vite build.
- Chrome DevTools inspected the actual component before and after using isolated mocked records:
  `fired` became `submitted`, retry/source details appeared, and the final 286-pixel panel had equal
  client and scroll widths. Preview files and the development server were removed afterward.
- Tracked protobuf output exactly matches the generated build output; source globs include the new
  database module for both Cargo and Bazel. Formatting and whitespace checks pass.

The first Cargo invocation needed permission to populate missing crates in the local Cargo cache.
The initial state-backed tests failed before assertions because the real AgentHub binary had not
been built; building the prerequisite resolved those fixture failures.

Default Bazel validation was attempted with:

```bash
bazel test //crates/agenthub-db:agenthub_db_tests //crates/agenthub-acp:agenthub_acp_tests --test_filter=reminder
```

It stopped during dependency loading: cached `bazel_skylib` lacked the package containing
`rules:common_settings.bzl`. No actions or tests executed. Bazel configuration was not changed;
this checkpoint makes no Bazel-pass claim.

## Follow-Ups

- Exact-head CI and review are separate from these local checks.
- Complete deployed standalone/Team ACP and remote node smoke checks in `docs/todo.md`.
- Recurring schedules, snooze, creation idempotency, quotas, finite obsolete-scope retries, and durable
  execution acknowledgments remain outside this change.
- Runtime queue acceptance is not an execution receipt: crash windows can duplicate submission or
  lose an already-submitted queued turn. No exactly-once claim is made.
