# Team Context/Memory Architecture For AgentHub Teams

## Status

- Stage: detailed design + roadmap merge (implementation pending)
- Version: v1
- Last updated: 2026-02-22

## Background

OpenClaw demonstrates a practical pattern for long-running agents:
deterministic prompt assembly, file-backed memory, and explicit lifecycle handling around compaction.

AgentHub Team runtime already has continuity Track 1/3 in progress (`team_member_continuity_state`, run-level continuity mode, continuity events), but does not yet define a complete file-backed context contract.

This document defines that contract in implementation-level detail.

## Goals

- Preserve process isolation while enabling cross-run cognitive continuity.
- Keep prompt assembly deterministic and cache-friendly.
- Move bulky observations out of prompt text into filesystem artifacts.
- Preserve operational traceability for retries, failures, and compaction decisions.

## Non-Goals

- Reusing process instances across runs.
- Introducing model fine-tuning or a custom prompt compiler.
- Replacing existing Team continuity Track 1 behavior.

## Core Principles

1. Stable prefix, dynamic tail.
2. Pointer-first memory (summary in prompt, details in files).
3. Append-only recovery trail (`decisions`, `errors`, `log`).
4. Explicit pre-compaction memory flush lifecycle.
5. Prompt mode layering (`full`, `minimal`, `none`) by role and task type.
6. Workspace-scoped context ownership (no cross-workspace context writes).

## Architecture Overview

### Components

- Prompt assembler
  - Builds final prompt from stable prefix + dynamic tail.
- Context memory writer
  - Persists artifacts and compact summaries into `.cache/context/run/<run_id>/`.
- Context index bridge
  - Stores artifact metadata for lookup/debug replay.
- Memory flush hook
  - Runs before compaction; emits lifecycle events.
- Team Events/Debug projection
  - Exposes continuity and memory-flush decisions.

### Data Flow

1. Runtime step executes and produces observations.
2. Oversized observations are offloaded into artifact files.
3. Dynamic tail receives compact summary + file pointers.
4. On compaction threshold, runtime executes memory flush hook.
5. Runtime emits explicit lifecycle events.
6. Next run can reuse bounded continuity envelope plus artifact pointers.

## Team Context Contract v1

### 1) Stable Prefix Contract

Must include:

- role charter and behavior boundaries;
- tool schemas and invocation contracts;
- static project constraints and output formatting rules.

Must exclude:

- timestamps;
- random identifiers;
- non-deterministic serialization order.

### 2) Dynamic Tail Contract

Must include:

- current objective (1-2 lines);
- next action (single concrete step);
- allowed actions block (`allow` + `deny`);
- compact state snapshot;
- evidence pointers;
- latest failure notes and retry intent.

### 3) Prompt Modes

- `full`
  - Used by leader/user-facing orchestration.
  - Contains full dynamic-tail sections.
- `minimal`
  - Used by worker/sub-agent execution.
  - Keeps only execution-critical sections.
- `none`
  - Emergency/diagnostic fallback only.

Default policy:

- leader path: `full`
- worker path: `minimal`

### 4) Workspace-Scoped Context Ownership

Each agent owns a separate context root under its own workspace:

- leader: `<leader_workspace>/.cache/context/...`
- worker-N: `<worker_workspace_N>/.cache/context/...`

Rules:

- an agent may only append/update files under its own context root;
- cross-agent memory sharing must go through explicit Team channels (mailbox/events/artifact pointers), not direct filesystem writes;
- artifact pointers should be recorded as metadata/events when another role needs to consume them.

Leader-specific requirement:

- leader workspace should be created as an empty coordination workspace;
- this workspace is reserved for context management artifacts, decisions, and orchestration traces.

## Filesystem Memory Design

### Memory Tiering Model (L0/L1/L2)

AgentHub Team memory uses a three-tier model so prompt budget stays bounded while durable context remains file-backed.

- `L0` Prompt Working Set (ephemeral, per step)
  - Purpose: immediate reasoning context for the next action.
  - Source: dynamic-tail fields (`objective`, `next_action`, `allow/deny`, latest failures, selected pointers).
  - Lifetime: one step/turn window; rebuilt each step.
  - Storage of record: none (derived view only).

- `L1` Run Episodic Memory (append-only, per run)
  - Purpose: preserve high-fidelity observations and execution trail for replay/recovery.
  - Source: tool outputs, step outputs, flush artifacts, debug traces.
  - Lifetime: bounded retention (configurable by run age/size).
  - Storage of record: `.cache/context/run/<run_id>/...` plus artifact index metadata.

- `L2` Durable Semantic Memory (curated, cross run)
  - Purpose: keep stable facts and durable decisions that should survive run rotation.
  - Source: promoted summaries from `L1` and explicit operator/leader edits.
  - Lifetime: long horizon with governance and periodic compaction.
  - Storage of record: `.cache/context/memory/*.md` (and optional local index for retrieval acceleration).

### Retrieval Priority And Budgeting

Context assembly should follow deterministic retrieval order:

1. Build `L0` first from current run-step state.
2. Retrieve recent `L1` slices by recency and step/member relevance.
3. Retrieve `L2` only for missing durable facts or policy constraints.

Prompt budget should be allocated by mode (guideline, tune by model window):

- `full`: prioritize complete orchestration context (`L0 + L1`, targeted `L2`).
- `minimal`: keep execution-critical context (`L0` first, small `L1`, sparse `L2`).
- `none`: diagnostic fallback with minimal `L0` only.

If budget pressure appears, drop lower-priority `L2` excerpts first, keep pointers, then compact `L1` snippets before reducing `L0`.

### Directory Layout

- `<agent_workspace>/.cache/context/`
  - `todo.md` (rewrite)
  - `state.md` (rewrite)
  - `decisions.md` (append-only)
  - `errors.md` (append-only)
  - `log.md` (append-only pointers)
  - `memory/` (durable `L2` memory root)
    - `profile.md` (stable role/profile facts)
    - `project_facts.md` (validated durable facts)
    - `decision_journal.md` (accepted cross-run decisions)
    - `open_questions.md` (explicit unresolved items)
- `<agent_workspace>/.cache/context/run/<run_id>/`
  - run-scoped `L1` artifacts

### Artifact Naming

Recommended pattern:

- `artifact-<seq>-<kind>.json`
- `artifact-<seq>-<kind>.txt`

Where:

- `<seq>` is monotonically increasing per run.
- `<kind>` examples: `tool_output`, `trace`, `payload`, `summary`.

### Offload Policy

When any observation exceeds configured threshold (`max_inline_chars` or `max_inline_lines`):

1. persist full payload into run artifact file;
2. persist metadata index row (run_id, seq, path, size, checksum, created_at);
3. inject summary + pointer in dynamic tail instead of full payload.

### Concurrency Rule

- Single writer per run context directory.
- If shared writes are possible, use lock file `<agent_workspace>/.cache/context/.lock`.

### Cross-Workspace Reference Contract

When one role needs artifacts from another role:

1. producer writes artifact in its own workspace context;
2. producer emits Team event with sanitized pointer metadata;
3. consumer resolves via Team API/event payload instead of direct filesystem traversal.

## Memory Flush Before Compaction

### Trigger

When used context approaches budget:

- `used_tokens >= context_window - reserve_tokens`.

Detailed design reference:

- `docs/journal/2026-02-22-team-memory-flush-spec.md`

### Lifecycle

1. Emit `memory_flush_started`.
2. Execute memory-flush attempt.
3. If durable notes were written, emit `memory_flush_persisted`.
4. If no durable notes were needed, emit `memory_flush_noop`.
5. Continue compaction.

### Failure Handling

If flush attempt fails:

- append failure note to `errors.md`;
- emit `memory_flush_failed`;
- continue with safe compaction fallback;
- keep error pointer in dynamic tail for next step.

## Event Contract

### Required Events

- continuity lifecycle:
  - `continuity_attached`
  - `continuity_reset`
  - `continuity_fallback`
- memory-flush lifecycle:
  - `memory_flush_started`
  - `memory_flush_persisted`
  - `memory_flush_noop`
  - `memory_flush_failed`

### Event Payload Minimum

- `team_id`
- `run_id`
- `member_id` (if applicable)
- `workspace_id` or `workspace_path` (sanitized form)
- `mode`
- `reason` (for fallback/failure)
- `artifact_pointer` (when relevant)
- `created_at`

## Security And Redaction

### Redaction Policy

Before writing artifact files or continuity summaries:

- apply secret-pattern redaction;
- apply token-like string masking;
- keep deterministic redaction markers.

### Storage Safety

- artifact files remain local and git-ignored (`.cache/`).
- debug views show bounded snippets and paths, not full sensitive payload by default.

## Observability

### Debug Surface

Team Events/Debug should show:

- prompt mode (`full`/`minimal`);
- continuity decision;
- memory-flush lifecycle;
- artifact pointer references.

### Metrics

Recommended counters/gauges:

- `context_artifact_write_total`
- `context_artifact_bytes_total`
- `memory_flush_total{status=...}`
- `context_compaction_total`
- `continuity_attach_total{status=...}`

## Compatibility And Rollout

### Backward Compatibility

- If filesystem memory is unavailable, runtime falls back to inline bounded summaries.
- Existing continuity Track 1 behavior remains default-safe.
- If workspace-specific context root is missing, runtime creates `<agent_workspace>/.cache/context` lazily.

### Rollout Steps

1. Land context contract in project docs and TODO.
2. Implement artifact writer + pointer-first dynamic tail.
3. Implement memory flush lifecycle and events.
4. Add Team Events/Debug rendering and tests.
5. Gate completion on CI evidence and repeated-run verification.

## Validation Plan

### Backend Tests

- prompt assembly mode split (`full` vs `minimal`);
- artifact offload threshold behavior;
- pointer serialization stability;
- memory-flush lifecycle branches (`persisted`, `noop`, `failed`);
- fallback behavior when artifact write fails.
- per-agent workspace isolation (leader/worker context roots do not overlap).
- cross-workspace artifact access requires Team event/API mediation.

### Frontend Tests

- Team Events/Debug rendering of memory-flush and continuity events;
- pointer rendering remains bounded and readable.

### End-To-End

- repeated run uses continuity + artifact pointers;
- compaction path preserves durable memory with explicit event trail.

## Delivery Roadmap (Merged)

This file now also serves as the continuity roadmap for Track 1/2/3 delivery sequencing.

### Track 1 (Active): Bounded Continuity Envelope

Scope:

- persist per-member continuity state after terminal step completion;
- attach continuity envelope on subsequent runs when mode is `inherit_recent`;
- keep envelope bounded and deterministic.

Acceptance criteria:

- no process instance reuse across runs;
- deterministic continuity attachment when enabled;
- explicit non-fatal fallback when continuity state is missing/corrupt/oversized.

### Track 2 (Design Pending): Long-Horizon Memory Architecture

Why deferred:

- retention/redaction/compaction/retrieval decisions must be finalized first;
- storage/latency budget and audit boundaries must be explicit before implementation.

Required outputs before coding:

- finalized `L0/L1/L2` ownership and write-path boundaries;
- deterministic `L1 -> L2` promotion policy;
- retrieval-budget policy by prompt mode (`full`/`minimal`/`none`);
- schema/index strategy + retention/compaction policy;
- rollback plan for flush/index/redaction failure paths.

Acceptance gate:

1. Tier boundaries are explicit and non-overlapping.
2. Promotion/demotion + retention rules have concrete operational examples.
3. Retrieval order/budget are deterministic and testable.
4. Failure fallbacks are explicit (`flush_failed`, index mismatch, redaction failure).

### Track 3 (Active): Continuity Controls And Observability

Scope:

- run-level continuity mode: `inherit_recent` / `reset`;
- bounded knobs (for example history item cap and char cap);
- lifecycle events: `continuity_attached`, `continuity_reset`, `continuity_fallback`.

Validation expectations:

- Team Events/Debug surfaces continuity decisions and reasons;
- repeated-run regression covers both attach and reset paths.

## Current Implementation Slice (2026-02-22)

Implemented:

- persistence table `team_member_continuity_state`;
- per-member continuity capture at step completion;
- orchestrator attach/reset/fallback event emission by continuity mode;
- actor runtime context can carry bounded continuity envelope.

Still pending:

- UI controls for continuity knobs in Team run surface;
- E2E for repeated-run continuity visualization in Team Events/Debug;
- Track 2 design sign-off and implementation tasks.

## Risks And Mitigations

- Risk: continuity payload bloat under prompt budget pressure.
  - Mitigation: strict bounds, truncation, pointer-first policy.
- Risk: stale continuity leading to incorrect behavior.
  - Mitigation: source metadata, explicit fallback/reset semantics.
- Risk: sensitive data persistence in memory artifacts.
  - Mitigation: deterministic redaction and dedicated coverage.
