# Native Capability Rollout

## Summary

Restore all previously deferred local native runtime requirements from the original slices 16-18:
controlled MCP/App/Mem tools, semantic outcomes, cross-process continuity, durable approval recovery
and assembled runtime acceptance. These are active implementation work. Reopening their scope does
not advertise capabilities that the pinned runtime cannot provide.

## Background

The user explicitly restored the remaining scope on 2026-10-05. This supersedes the deferral in the
[ACP integration checkpoint](2026-09-21-acp-rollout-integration.md). ACP slices 1-15 and their
revocation follow-up are already integrated through PRs #1169 and #1174. PR #1168 merged the existing
native transport, durable receipts/events, fresh activation path and subsequent correctness fixes.

The starting main revision is `fb8720638c9ec2f6d5d4a499445bb748bfd897d6`. The native adapter still pins
upstream `6f489462251b73e1695bb22a59d2ece59ba26a21`. Source inspection of the later committed upstream
revision `bc91935e` also finds no advertised controlled MCP registration or session resume, and its
handshake still reports `approval_persistence: false`. Shared request types do not establish an
implemented stdio operation. The upstream working directory contains independent image-input work;
that work is not an implementation baseline for this rollout.

## Scope

The target is the complete local native path in the
[direct runtime contract](../features/rara-direct-integration.md), behind the shared scheduler,
journaled tool proxy, canonical task/IM stores and verified process supervision. Role/Card/task
binding, stable task memory prefixes and nested-subagent authority boundaries remain part of
acceptance. ACP remains a supported integration baseline throughout.

Remote placement, non-Linux guardians, standalone Mem scope, App authoring UI and cross-owner
consent remain outside this work. Historical checkpoints retain their original dated decisions;
the current TODO and canonical specs describe the restored scope.

## Key Decisions

- Reuse the existing journaled MCP proxy for native MCP, App and Mem access. Do not create a second
  tool authorization or receipt system in the native adapter.
- Implement and qualify upstream controls before changing the pinned revision or admitting their
  capabilities. Preserve explicit rejection for unsupported operations on older runtimes.
- Keep activation identity, local process/session identity and provider continuity distinct. A
  recovered provider session never restores an expired activation fence or execution credential.
- Persist approval ownership and decision receipts separately from execution certainty. A restart
  or an unknown acknowledgement does not authorize replay of a decision or side effect.
- Deliver new reviewable PRs, with explicit stacked dependencies while prerequisite PRs remain
  open. Preserve unrelated upstream work and validate the exact committed provider revision used
  by each adapter change.

## Delivery Plan

### 1. Controlled tool sources

Scope: upstream source registration and actual tool-registry admission, followed by adapter
bootstrap through the existing proxy for MCP, Apps and Mem. Session-scoped registration reuses
outer authorization and revocation instead of ambient extension configuration.

Entry: the merged source-registration and durable-receipt baseline. First prove a minimal upstream
registration can invoke one controlled fixture tool, retire it, and reject a foreign session.
Then pin that committed protocol and extend native bootstrap. Exit: acceptance covers credential
isolation, revocation during execution, registration/ACK ordering and no uncertain-write replay.

### 2. Semantic outcomes

Scope: explicit upstream semantic events and their mapping to fenced loop outcomes and clarification
waits. Structured events provide attributable reasons; diagnostic text is not an outcome protocol.

Entry: define and prove the event contract against the actual producer. Exit: compatible work,
non-actionable mismatch and clarification waits are distinct; duplicates, stale generations and
ordinary terminal turns cannot overwrite an authoritative finish. This work can be prepared after
the protocol baseline is established without depending on continuity recovery.

### 3. Cross-process continuity

Scope: supported provider-session restore into a new local launch and fenced activation. Retained
canonical tasks, mailbox identity, task memory prefixes and history remain outside process lifetime.

Entry: controlled source bindings can be reconstructed for a new session. Prove a real process
restart retains provider history while the outer scheduler admits only the new owner. Exit: verified
old-owner cleanup, configuration compatibility and source reauthorization precede replacement;
stale credentials and silent fresh-session fallback are rejected.

### 4. Durable approval recovery

Scope: durable pending interactions, restored ownership and explicit answer receipts across process
loss. Reuse the existing live approval path and outer fences rather than inventing implicit approval.

Entry: cross-process continuity and durable upstream pending-state recovery. Exit: crash-boundary
cases before/after pending-state commit, answer admission, ACK and tool execution preserve exact
ownership; duplicate, stale or uncertain answers cannot execute a replacement interaction. Advertise
approval persistence only after the provider and adapter satisfy that contract together.

### 5. Assembled acceptance

Scope: installed runtime and configured-provider qualification of the combined behavior, followed
by operator documentation and applicable CI on each final PR head.

Entry: all four capability gates have direct evidence. Exit: leader/worker activations use controlled
tools and scoped knowledge, survive restart and pending approvals, map outcomes correctly, preserve
history after browser/process exit, and keep nested subteams inside the outer activation's authority.
Record the exact binary revision, provider/model, test mode and evidence; deterministic fixtures do
not by themselves establish configured-provider acceptance.

## Controlled Connection Checkpoint

The first implementation component is published in
[upstream PR #1057](https://github.com/linkerdog/rara/pull/1057), commit
`457fafeac07a55ad7681ac305b319154043fb2f0`, based on upstream main
`1c063f4e9b474473a259af2f544fc786dfd86de4`. An isolated checkout preserves the
independent image-input work.

The component owns a callable stdio connection, bounds raw incoming frames and
complete catalogue discovery, sends one request without automatic SDK retries,
and explicitly reaps its direct child. Interrupted retirement retains uncertainty.
Existing upstream discovery reuses the same connection and cleanup path.

Evidence includes 12 focused tests with real stdio children and frame-boundary
checks, focused warnings-denied Clippy and the upstream repository's all-target
Clippy commit hook. Default local Bazel validation timed out in dependency fetching
before any compilation or test process; the foundation PR's remote Bazel, build
and Clippy checks subsequently passed. Its full test job failed the unchanged
external-editor PTY input case on both attempts. That case passed once in local
isolation; the CI failure remains unresolved and is a merge gate.

This component is the transport foundation. The next component supplies the
session registry and executable controls described below.

## Controlled Session Checkpoint

[Upstream PR #1058](https://github.com/linkerdog/rara/pull/1058), commit
`aa3a6e927ed77ba787625497484a90f0ca0eaf4e`, adds explicit session-owned source
registration, query and removal. It targets the foundation branch while #1057
remains open, so the review diff contains only session integration.

The session actor admits complete namespaced catalogues atomically, uses an
explicit launch environment and workspace, and fences each call to the owning
session. Removal invalidates retained tool handles before waiting for child
retirement. Uncertain source cleanup blocks further admission and successful
shutdown receipts. Dynamic sources require host opt-in and cannot widen frozen
profiles or session-stable schemas; read-only modes do not grant execution
authority based on source annotations.

Evidence includes 7 focused source/native tests, 10 stdio tests, a dedicated
read-only-mode test, all-target warnings-denied Clippy and formatting. The actual
binary passed all 6 smoke scenarios, including a local provider calling a real
controlled source and observing its retirement before semantic shutdown. These
fixtures do not establish configured-provider acceptance.

## Controlled Proxy Adapter

The adapter now pins `460778e10c2ce01f2dc6b6de57d4d25c609f9c40` and negotiates
the complete controlled-source method set before creating a session with proxy
mounts. The existing launch configuration supplies the same local proxy descriptors
to ACP and native transports. Source receipts use the existing durable intent and
event-prefix barrier; no upstream launch secret enters the receipt store. Source
events project safe IDs and bounded counts. The `native-loop-v3` digest records
the changed bootstrap contract.

The native preflight now applies the existing App/Mem binding rules, with concrete
source capability checks at runtime startup. An older runtime remains usable for
unbound sessions and rejects a bound launch before its entry prompt. The adapter
does not introduce a second proxy, credential store or side-effect journal.

The protocol fixture was captured from a private copy of the committed candidate's
rebuilt binary with debug symbols stripped,
SHA-256 `2f50d4ea5f22e17420aa41f13fdad153673d992a472ad1927e163bf81996f97e`.
The capture covers handshake, session creation, duplicate creation receipt and
semantic shutdown with stdin open. The 45 protocol tests, database source-receipt
test, 5 managed source-registration tests and 8 native loop regression tests pass.
The combined actual-process App/Mem fixture exposed a stale upstream receipt:
prompt registration published event 2 but acknowledged sequence 1. Dedicated
upstream source and query regressions both reproduced it. The actor now refreshes
its snapshot before resolving the control, preserving the current phase and the
outer adapter's strict durable-prefix gate. The actual proxy then rejected the SDK
default `2026-07-28` initialize request, which belongs to a different lifecycle.
The owned connection now explicitly requests `2025-11-25`; a strict child fixture
reproduces the mismatch before the fix and all 13 connection tests pass afterward.

The combined actual-process fixture now passes both successful-write and
lost-response cases using the real native process, two real proxy shims and local
App/Mem/model endpoints. It verifies source receipt persistence, credential
isolation, Mem scope rejection and injection, immediate App revocation, exactly
one upstream dispatch for an uncertain write, durable `outcome_unknown`, and
closed runtime history. The existing actual-process dispatch/report/acceptance
regression also passes with the candidate, including process exits and nested work.
This is deterministic assembled evidence, not configured-provider acceptance.

The two corrections are local commits `f2128e69` and `460778e1` on the upstream
registry branch. Publication to upstream PR #1058 is pending explicit destination
authorization after automatic approval review rejected that external push. The
adapter is reviewable as a draft until its pinned source revision is published.

Cross-process continuity, semantic outcomes, approval persistence and configured
provider acceptance remain open. Both upstream PR test runs expose the same
unchanged external-editor PTY failure. Ten isolated local repetitions and the full
local library test binary (2131 tests at `aa3a6e92`) passed; the CI failure is still not reproduced, and its
remaining gate has not been waived.

## Validation

The initial checkpoint reopened scope; the controlled-source adapter now changes
bootstrap, protocol pinning and preflight behavior. No database migration is
required. Review the diff and changed local documentation links. Focused commands
include:

```sh
git diff --check
cargo test -p agenthub-rara
cargo test -p agenthub --lib agent::manager::loop_launch::tests::native
cargo test -p agenthub-db runtime_events
cargo clippy -p agenthub -p agenthub-rara -p agenthub-db -p agenthub-acp --all-targets --no-deps -- -D warnings
cargo fmt --all --check
cargo build -p agenthub --bin agenthub
AGENTHUB_RARA_TEST_BINARY=/absolute/path/to/pinned/runtime cargo test -p agenthub --lib controlled_proxy_sources_enforce_scope_revocation_and_uncertainty -- --ignored
```

Select additional tests from the milestone's actual boundary; the commands above do not replace
upstream executable proofs, process-loss tests, browser checks for UI changes or final runtime
acceptance. Keep Bazel validation on the default configuration.

## Follow-Ups

The five open milestones in [the transition TODO](../todo.md) are the active delivery checklist.
Complete the controlled-source protocol proof first, then publish the corresponding upstream and
adapter changes with their evidence. The prior user deferral is no longer a blocker for this work.
