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

## Review Follow-Up

PR #1118's asynchronous-render review is addressed in both component tests. Requests now remain
pending until explicitly resolved and awaited inside `act`, with loading assertions before settlement.
The identity-change test checks the new agent's response before delivering the stale response and
checks the polling deadline plus the absence of overlapping refreshes while a request is pending.

Validation for this test-only follow-up:

- `npm exec vitest -- run src/components/agent_time_triggers_panel.test.tsx`: both tests passed.
- `npm exec tsc -- --noEmit` and focused ESLint: passed.
- `git diff --check`: passed.

At implementation commit `f6a2920d8cb7a5b15f5948ba8ea0d42111bce17e`, the remote Bazel coverage job
executed all 19 test targets successfully, including the root, ACP, and database targets. Codecov
still reported 2.56% patch coverage and failed its patch/project checks. The job's result list showed
coverage files for only three dependent test targets, with none listed for the root, ACP, or database
targets themselves. The coverage command omits `--instrument_test_targets`, while Cargo coverage is
push-only. This indicates an instrumentation gap requiring separate validation; passing tests alone
do not establish coverage completeness. This follow-up changes neither Bazel configuration nor
coverage thresholds.

## Full Review Follow-Up (2026-09-13)

The review covered all 30 original changed files and all PR review records. The asynchronous panel
thread was already resolved. The follow-up synchronizes `main` and addresses these remaining edges:

- Claim ordering uses `MAX(fire_at, next_attempt_at)` so overdue retries are not indefinitely
  displaced by new rows whose retry timestamp defaults to zero. The regression pairs an overdue
  retry with a newer due reminder and limits each claim to one row.
- Claims clear the previous `last_error`, restoring the original lifecycle semantics and preventing
  an active retry from retaining the inspector's red failure state. The backoff test covers both
  the retained failure before claim and its removal from returned and persisted records afterward.
- Manager validation carries a typed internal error. HTTP maps it to `400` and gRPC to
  `INVALID_ARGUMENT`, while operational errors keep their existing mapping. Duplicate transport
  delay-range validation is removed; RPC retains the legacy absolute-deadline error and rejects
  mutually exclusive scheduling fields.
- Local and remote source references share one normalization path. Runtime snapshot lookup no
  longer takes an unused reference argument.

The latest reviewed `main` coverage run,
[34092453307](https://github.com/hawkingrei/agenthub/actions/runs/34092453307), failed because the
distributed blackbox test could not find `target/llvm-cov-target/debug/agenthubd`. A separate prepared
workflow patch enables fresh PR coverage and uses the documented `show-env`, clean, build, test, and
report sequence, building all workspace binaries before the tests. GitHub rejected its initial push
because the OAuth credential lacks `workflow` scope, so the code fixes were published separately.
After PR #1118 merged, the repository's existing SSH identity was authorized and authenticated for
the workflow follow-up below. Bazel configuration and coverage thresholds remain unchanged.

Validation for this follow-up:

- `cargo test --locked -p agenthub --lib reminder`: retry ordering, lease/error lifecycle, scope,
  timeout, standalone identity, and gRPC validation boundaries.
- `cargo test --locked -p agenthub --lib time_trigger`: HTTP invalid-input classification and
  existing create/list/cancel compatibility.
- `cargo test --locked -p agenthub-db -p agenthub-acp reminder`: storage migration and deferred ACP
  policy boundaries.
- `npm exec vitest -- run src/components/agent_time_triggers_panel.test.tsx`, TypeScript, ESLint,
  production Vite build, formatting, and whitespace checks.
- The local results are 13 reminder tests, 10 time-trigger tests, three ACP/DB tests, and two panel
  tests passing. The HTTP/RPC regression retains the legacy past-deadline error text and lookup
  precedence. A direct SQLite comparison also reproduced the old retry-ordering failure.
- The mTLS remote reminder test passed after allowing its ephemeral loopback listener; the sandbox
  initially rejected binding the port before any protocol assertions.
- Fresh PR CI must validate this code head. The separate coverage workflow repair still needs
  publication and a successful report upload.

Deployed provider smoke checks and execution acknowledgments remain the existing follow-ups.

## CI Workflow Repair (2026-09-13)

The workflow follow-up enables Cargo coverage on PRs and builds instrumented workspace
binaries before running the complete workspace test set. The sequence follows the
[cargo-llvm-cov external-tests contract](https://github.com/taiki-e/cargo-llvm-cov/tree/v0.9.1#get-coverage-of-external-tests).
Successful CI execution and fresh coverage upload must be confirmed on the follow-up PR.

The code head's initial Actions runs failed before creating jobs with a GitHub internal-error
annotation. Reruns reached the normal build/test steps. The S3 fixture then failed before testing
because Docker Hub denied the existing `minio/minio` image pull. The workflow uses MinIO's official
`quay.io/minio/minio` repository with the same fixed release tag. `docker manifest inspect
quay.io/minio/minio:RELEASE.2025-06-13T11-33-47Z` successfully resolved the OCI index, including
`linux/amd64`. This validates image availability, not the unexecuted S3 test job.

The first run of follow-up PR #1134 successfully started the Quay image, then failed while fetching
`mc` because `dl.min.io` returned HTTP 410. Bucket creation now uses curl's AWS SigV4 support with
the existing fixture credentials and region, removing the separate client download. The job starts
a fresh MinIO container, so this step creates the fixture bucket once before the S3 tests.

Local all-target Clippy with `-D warnings` and comparison of tracked/generated protobuf output also
passed for the code follow-up. Remote coverage and S3 results remain required follow-up evidence.

## Bazel Fixture Follow-Up (2026-09-13)

At code head `2e9f1219`, [Bazel Test (Crates)](https://github.com/hawkingrei/agenthub/actions/runs/34749297807/job/103703411823)
failed one of 153 Codex ACP tests while executing a freshly written fake runtime:
`ExecutableFileBusy` (`ETXTBSY`, error 26). The failure occurred before protocol assertions. The
other 17 crate targets passed.

The fixture had the [concurrent fork descriptor race](https://github.com/rust-lang/rust/issues/114554):
a sibling process can inherit an open writable script descriptor and retain it beyond the writer's
local close. The fixture now locks the writer, closes it, and acquires a shared lock through a new
read-only descriptor. This waits for inherited writable descriptors to close before execution.
All executable script fixtures use the same helper. Production runtime behavior and Bazel
configuration are unchanged.

Validation:

- A focused fixture regression creates and immediately executes scripts on four concurrent
  threads, with 64 distinct scripts per thread.
- An extracted harness reproduced the original `ETXTBSY` failure on its sixth repetition; the
  repaired helper completed 100 repetitions (25,600 script executions).
- `cargo test --locked -p agenthub-codex-acp-runtime --lib`: all 154 tests passed, including the
  original failing initialization test and the new concurrent fixture regression.
- `cargo clippy --locked -p agenthub-codex-acp-runtime --tests -- -D warnings`: passed.
- `cargo fmt --all --check` and `git diff --check`: passed.

The Codecov upload records initially identified both Rust reports as carried forward from earlier
commits. All required checks subsequently passed on `666fac7d`, including the repaired Bazel crate
suite, and PR #1118 merged as `d399edee`. Fresh Cargo coverage and MinIO fixture execution remain
the workflow follow-up above.
