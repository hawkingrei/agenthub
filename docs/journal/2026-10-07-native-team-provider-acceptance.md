# Native Team Provider Acceptance

## Summary

Add opt-in configured-provider checks for native Team semantic outcomes, the controlled App/Mem
proxy, a dispatch/report/acceptance cycle with private native children, and database/manager
restart with durable conversation and approval recovery. The fixtures use synthetic work and
isolated local state. External provider qualification remains pending.

## Background

Deterministic native-process tests already cover these boundaries. Standalone configured-provider
restart and approval checks do not establish Team routing, controlled tools, or nested identity
behavior. The additional fixtures make those remaining checks explicit and repeatable.

## Scope

- Canonical `no_actionable_work` for work outside the member Card, and `waiting/input` plus a
  durable clarification message when required task inputs are missing. Neither case may execute
  worker tools or complete the canonical task.
- App/Mem source negotiation, denied foreign memory scope, injected authorized scope, immediate
  App revocation and durable uncertain-write recording without another upstream effect.
- Three sequential activations dispatch, report and accept one canonical task through the signed
  control CLI. Each activation starts a native child that creates a private task; those tasks and
  child identities must remain outside canonical Team state.
- Reopen a production-schema SQLite database under a new manager, preserving the canonical task,
  memory prefix and mailbox identity while runtime/local-session identities and generations change.
  Recover a pending approval under a new callback, or explicitly review an uncertain append before
  later durable input. The append must occur once, and ordinary provider completion must leave the
  canonical task open.

## Key Decisions

- External runs require both `AGENTHUB_RARA_TEST_BINARY` and an explicit private
  `AGENTHUB_RARA_PROVIDER_CONFIG`. Tests copy the selected provider configuration into a private
  fixture directory and remove it during cleanup. They never edit the user's configuration.
- Test role additions are bounded task inputs and output constraints. Production role prompts,
  runtime tails, managed skills and plugin entrypoints are unchanged.
- The production Team launcher supplies ambient extension and memory isolation. The fixture must
  not append duplicate CLI flags.
- Every shell approval must match the prepared command, workspace, empty environment overrides,
  foreground execution and escalation request. Only one allow-once answer is permitted per
  activation. Unrelated or repeated requests fail the fixture.
- Controlled tools remain behind real local proxies and the operation journal. The App fixture
  revokes its grant before returning its first write result, so a later denied call does not depend
  on a scripted model triggering the revocation.
- The lifecycle script performs predetermined test actions. Success proves native model/tool
  integration and authority boundaries, not autonomous planning quality.
- Recovery task instructions live in the current canonical task summary. Updating that summary
  must not change role configuration or the task memory prefix. Approval and recovery oracles use
  retained receipts, actual assistant output and the isolated effect file rather than model claims.

## Validation

The three new entrypoints pass against the qualified native candidate with a localhost-only
deterministic model: two semantic cases, successful and uncertain controlled-tool cases, and the
three-activation Team cycle. The cycle confirms three private child task artifacts, unchanged outer
agent identity inventory and coordinator acceptance of the canonical task. The production control
CLI, proxies, journal and native subprocesses are used throughout.
The candidate SHA-256 is `70c42d4622ded1c04103a0d7a2087dbe06083930d0e0646bb80c6f28f8a32dac`.

The existing native lifecycle, controlled-proxy and semantic-guard process suites also pass
(three regression tests). Local logs are `/tmp/agenthub-configured-team-loopback-final.log`,
`/tmp/agenthub-configured-team-loopback-mcp-final.log` and
`/tmp/agenthub-configured-team-deterministic-regression.log`. Loopback payload evidence lives under
`target/loop-review-validation/configured-team-loopback-20261007`; captured requests did not include
the controlled services' private credentials or endpoints.
Root `cargo clippy -p agenthub --lib --tests -- -D warnings`, `cargo fmt --all --check`,
`git diff --check` and 131 local documentation links pass. All task-owned fixture services stopped.

The recovery follow-up adds three cases for random-code recall, restored approval and uncertain
effect review across database/manager restarts. All three final cases pass against the same native
candidate and a localhost-only deterministic model; the three affected existing continuity
process regressions also pass. The final recovery run made 13 local model requests, including
seven tool-free reentry checks, and no external provider requests. Old approval callbacks are
rejected after the new callback or recovery entry appears. Another database/manager restart
confirms that review creates no pending continuation. Root Clippy with warnings denied,
formatting, whitespace and all 131 documentation links pass. The owned model service stopped.

No new external configured-provider Team result has been recorded. Automatic approval rejected the
external semantic check before execution because the task, role and project-prompt payload to the
configured destination had not been specifically authorized. The unused private configuration
snapshot was removed. Local dummy-model results do not close that qualification gate.

After authorizing the selected provider and synthetic payload, run these opt-in tests with the
two environment variables above:

```bash
cargo test -p agenthub --lib native_process::configured:: -- --ignored --test-threads=1
cargo test -p agenthub --lib agent::manager::loop_launch::tests::native_process::mcp::configured::configured_provider_controlled_tools_preserve_scope_and_uncertainty -- --exact --ignored
cargo test -p agenthub --lib native_process::continuity::configured:: -- --ignored --test-threads=1
```

The proxy parent supplies isolated dummy upstream credentials to its child. Do not run the child
entry directly or run every ignored test indiscriminately.

## Follow-Ups

- Run the opt-in Team cases against the authorized configured provider and record the model,
  candidate identity and bounded results separately from deterministic evidence.
- Qualify the Team continuity/recovery cases with the authorized provider and complete
  installed-runtime qualification.
- Publish the producer changes after destination authorization and complete applicable CI.

Contract: [direct runtime integration](../features/rara-direct-integration.md).
Earlier evidence: [standalone native continuity](2026-10-07-native-standalone-continuity.md).
