# Team Workspace Memory Contract

## Problem

AgentHub Team runtime already uses workspace-local `.cache/context/` state, project-local
`.agenthubmemory/` notes, and pointer-first prompt guidance. What is still easy to drift is the
exact contract between these surfaces: which data belongs in runtime continuity, which data belongs
in durable project memory, which files act as stable indexes, and how prompt assembly should point
to filesystem-backed detail instead of replaying it inline.

Without one stable contract, leader and worker prompts, runtime continuity code, role skills, and
Team docs can each describe memory ownership slightly differently.

## Scope

- Workspace-local memory ownership for Team leader and worker sessions.
- The v1 boundary between `.cache/context/` and `.agenthubmemory/`.
- Stable index files, append-only records, and run-scoped artifacts.
- Prompt-tail interaction with file-backed memory.

## Non-Goals

- Defining retrieval ranking or vector search.
- Replacing current Team continuity or ACP resume behavior.
- Introducing cross-workspace shared storage.

## Architecture

### 1) Memory Surfaces

AgentHub Team runtime uses two distinct filesystem memory surfaces:

- `.cache/context/`
  - Runtime-owned context state.
  - Stores prompt-facing indexes, continuity state, append-only recovery trails, and run-scoped
    artifacts.
  - Must remain bounded, pointer-first, and safe to rebuild from runtime state plus append-only
    records.
- `.agenthubmemory/`
  - Agent-authored durable project memory.
  - Stores longer-lived task ledgers, execution journals, reusable notes, and project-facing
    follow-up material.
  - Exists primarily for workers operating inside a concrete repository; leader coordination
    workspaces usually do not need it.

### 2) Ownership Model

- Every agent writes only inside its own workspace.
- `.cache/context/` is always workspace-local and never shared by direct filesystem writes.
- `.agenthubmemory/` is also workspace-local; it is not a team-wide shared notebook.
- Cross-member sharing goes through Team channels, mailbox payloads, or artifact pointers, not
  direct path traversal into another workspace.

### 3) Prompt Contract

Prompt assembly should treat filesystem memory as the source of durable detail:

- keep the prompt tail small:
  - current objective
  - next action
  - allowed actions
  - compact blocker/failure notes
  - selected artifact pointers
- keep large observations and long execution history out of inline prompt text
- point to `.cache/context/run/<run_id>/...` or `.agenthubmemory/...` artifacts when deeper detail
  is needed

## Contracts

### 1) Workspace Root And `.cache/context/` Contract

The v1 layout separates workspace-root coordination artifacts from runtime-owned
`.cache/context/` state:

```text
<agent_workspace>/
  AGENTS.md
  TODO.md
  .cache/
    context/
      state.md
      decisions.md
      errors.md
      log.md
      memory/
        profile.md
        project_facts.md
        decision_journal.md
        open_questions.md
      run/
        <run_id>/
          artifact-<seq>-<kind>.json
          artifact-<seq>-<kind>.txt
```

File ownership rules:

- `AGENTS.md`
  - Stable routing/index pointer file for the current role and phase.
  - Should stay concise and point to deeper docs/skills/artifacts instead of copying them inline.
- `TODO.md`
  - Runtime-visible current work index for the workspace root.
  - May be rewritten as state advances.
- `state.md`
  - Compact current-state snapshot.
  - Rewrite-friendly; not append-only.
- `decisions.md`
  - Append-only accepted decisions and rationale.
- `errors.md`
  - Append-only failure trail with short summaries and pointers.
- `log.md`
  - Append-only operational trace and pointer log.
- `memory/profile.md`
  - Stable role/profile facts that help future runs re-establish identity and durable working
    assumptions.
- `memory/project_facts.md`
  - Validated durable project facts and constraints promoted from run artifacts.
- `memory/decision_journal.md`
  - Curated cross-run decisions promoted from run artifacts.
- `memory/open_questions.md`
  - Durable unresolved questions that should survive run rotation until answered.
- `run/<run_id>/...`
  - High-fidelity run-scoped artifacts and replay/debug material.

### 2) `.agenthubmemory/` Contract

The v1 worker-facing layout is:

```text
<worker_workspace>/.agenthubmemory/
  TODO.md
  journal/
  note/
  scratch/
```

File ownership rules:

- `TODO.md`
  - Durable project/task ledger for the worker's concrete repository work.
  - Holds resumable local tasks that matter even when the active run changes.
- `journal/`
  - Chronological execution notes, checkpoints, and handoff summaries.
- `note/`
  - Reusable lessons, heuristics, command snippets, and project-specific reminders.
- `scratch/`
  - Temporary research/output staging that is useful beyond one turn but not yet promoted into a
    durable note.

Leader workspaces may omit `.agenthubmemory/` entirely when they stay as empty coordination
workspaces.

### 3) Tiering And Promotion Contract

AgentHub uses a three-tier memory model:

- `L0`
  - Prompt working set only.
  - Rebuilt each turn; not stored as a durable file.
- `L1`
  - Run episodic memory under `.cache/context/run/<run_id>/...`.
  - Stores high-fidelity observations, tool outputs, flush artifacts, and replay/debug evidence.
- `L2`
  - Curated durable memory under `.cache/context/memory/*.md` and, for worker project work,
    `.agenthubmemory/`.
  - Stores facts and notes that should survive run rotation.

Promotion rules:

- Promote into `L2` only when the content is durable, specific, and still useful after the current
  run ends.
- Keep `L1` append-only and high fidelity.
- Prefer promotion by summary plus source pointer instead of copying raw logs into durable notes.

### 4) Offload And Pointer Contract

When an observation is too large for prompt text:

1. write the full payload under `.cache/context/run/<run_id>/...`
2. record a concise summary in prompt-visible state or append-only logs
3. carry a stable pointer/path reference instead of the raw payload

The same rule applies to worker project memory:

- `.agenthubmemory/note/` may hold concise summaries and reusable notes
- oversized raw evidence should still live under `.cache/context/run/<run_id>/...`, with
  `.agenthubmemory/note/` or `.agenthubmemory/journal/` pointing to it when needed

### 5) Role-Specific Rules

- Leader
  - Default workspace is an empty coordination workspace.
  - Uses workspace-root `AGENTS.md` and `TODO.md` for coordination indexes, plus `.cache/context/`
    for runtime continuity, append-only trails, and pointer-backed run artifacts.
- Worker
  - Uses `.cache/context/` for runtime continuity and run-scoped evidence.
  - Uses `.agenthubmemory/` for durable repository/task memory that survives one run and remains
    useful for future local execution.

### 6) Recovery And Flush Contract

- Pre-compaction flush writes new durable evidence into `.cache/context/run/<run_id>/...` before
  reducing prompt-visible state.
- Flush outcomes should remain explicit (`persisted`, `noop`, `failed`) so recovery trails stay
  auditable.
- Durable facts promoted from flush output should be written into `.cache/context/memory/*.md` or
  `.agenthubmemory/note/` as concise summaries with source pointers.

### 7) Rolling Upgrade Compatibility Contract

Filesystem-backed Team memory must support mixed-version runtime deployments without requiring one
atomic workspace rewrite.

Compatibility rules:

- Treat machine-read index files as versioned file protocols, not ad-hoc prompt dumps.
- Prefer additive evolution:
  - add new fields or new pointed-to files first;
  - keep existing stable pointer fields readable for at least one compatibility window;
  - remove or rename fields only after every supported reader no longer depends on them.
- Readers must be more tolerant than writers:
  - new readers should accept missing optional fields, unknown extra fields, and older field
    shapes when the core pointer contract still resolves;
  - writers may emit the latest schema, but should preserve compatibility-facing fields until the
    previous reader generation is retired.
- Do not require startup-time full-workspace rewrites or destructive migrations just to load
  context state.

Machine-read file requirements:

- Stable index files such as `state.md` should declare:
  - `schema_family`
  - `schema_version`
- Run-scoped artifacts and machine-read markdown or JSON notes should also carry self-describing
  schema metadata whenever a runtime parser depends on their structure.
- `state.md` is the compatibility-facing index for current runtime context, not the full-fidelity
  state source; detailed continuity or replay material should live behind stable pointers under
  `.cache/context/run/<run_id>/...`.

Migration strategy:

1. land a backward-compatible reader first;
2. start writing the new schema or new pointed-to file shape;
3. keep compatibility fields or dual-read support during the rollout window;
4. remove legacy fields only after the old reader path is no longer supported.

Practical consequences:

- Pointer paths are the primary stability boundary; index files should change more slowly than the
  deeper artifact payloads they reference.
- Append-only files (`decisions.md`, `errors.md`, `log.md`) should accept entry-shape evolution by
  appending new records rather than rewriting historical entries in place.
- If a field rename is semantically necessary, prefer a staged dual-read or dual-write window over
  a one-shot replacement.

## Operational Notes

- Treat prompt text as the bounded working set, not the durable notebook.
- Prefer stable index files plus append-only trails over large rewrite-heavy state dumps.
- Keep file names predictable so agents can navigate them cheaply.
- Avoid creating new parallel memory roots unless the contract above proves insufficient.

## Open Risks

- The exact line between `.cache/context/memory/*.md` and `.agenthubmemory/` may still need tuning
  once more long-running Team sessions are observed in practice.
- Retrieval/ranking policy for promoted memory remains intentionally out of scope for v1.

## Source Journals

- [2026-02-22-team-context-memory-architecture.md](../journal/2026-02-22-team-context-memory-architecture.md)
- [2026-02-22-team-memory-flush-spec.md](../journal/2026-02-22-team-memory-flush-spec.md)
- [2026-04-10-team-prompt-tail-slimming.md](../journal/2026-04-10-team-prompt-tail-slimming.md)
- [2026-04-10-runtime-context-identity-compaction.md](../journal/2026-04-10-runtime-context-identity-compaction.md)
- [2026-04-10-team-workspace-memory-contract.md](../journal/2026-04-10-team-workspace-memory-contract.md)
- [2026-04-18-team-memory-index-rolling-upgrade.md](../journal/2026-04-18-team-memory-index-rolling-upgrade.md)
- [2026-04-18-team-memory-index-schema-metadata.md](../journal/2026-04-18-team-memory-index-schema-metadata.md)
