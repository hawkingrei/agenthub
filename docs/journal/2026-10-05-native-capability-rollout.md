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
- Deliver new reviewable PRs from merged main. Preserve unrelated upstream work and validate the
  exact committed provider revision used by each adapter change.

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

The main adapter still pins the earlier revision and retains its unsupported
MCP-source preflight. The next change must negotiate the committed controls,
register the already-authorized proxy mounts through durable receipts, and prove
App/Mem isolation, revocation and uncertain-write behavior before removing that
rejection. Cross-process continuity, semantic outcomes, approval persistence and
assembled acceptance remain open.

## Validation

This checkpoint changes scope and acceptance documentation. It does not modify runtime behavior,
protocol constants, schema, capability advertising or existing preflight rejection. Review the diff
and changed local documentation links. Relevant commands for later implementation slices include:

```sh
git diff --check
cargo test -p agenthub-rara
cargo test -p agenthub --lib agent::manager::loop_launch::tests::native
cargo test -p agenthub-db runtime_events
cargo fmt --all --check
```

Select additional tests from the milestone's actual boundary; the commands above do not replace
upstream executable proofs, process-loss tests, browser checks for UI changes or final runtime
acceptance. Keep Bazel validation on the default configuration.

## Follow-Ups

The five open milestones in [the transition TODO](../todo.md) are the active delivery checklist.
Complete the controlled-source protocol proof first, then publish the corresponding upstream and
adapter changes with their evidence. The prior user deferral is no longer a blocker for this work.
