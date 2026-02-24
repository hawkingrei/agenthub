# Team Memory Flush Specification (No-Code Phase)

## Status

- Stage: design-only (implementation not started)
- Date: 2026-02-22
- Scope: Team context/memory flush before compaction

## Problem Statement

Team runs currently persist bounded continuity snapshots, but there is no explicit pre-compaction flush protocol.
Without a flush protocol, durable context can be silently dropped when context budget pressure appears.

This document defines a concrete memory-flush design that can be implemented without changing process-isolation rules.

## Design Goals

- Preserve Team process isolation (no session/process reuse).
- Persist durable context before compaction.
- Keep prompt payload bounded via pointer-first artifact storage.
- Guarantee deterministic and auditable flush lifecycle.
- Maintain workspace-scoped context ownership.

## Non-Goals

- Long-horizon semantic retrieval ranking logic.
- Cross-workspace direct file sharing.
- Runtime model-specific tokenizer dependence in v1.

## Trigger Model

Memory flush is triggered by three paths.

1. Soft threshold trigger
- Condition: estimated used budget reaches warning threshold.
- Formula:
  - `estimated_used >= context_window - reserve_tokens`
  - estimate can use a deterministic char-based approximation in v1.

2. Hard failure trigger
- Condition: runtime receives context-overflow style errors.
- Typical signals:
  - provider overflow message
  - context-length exceeded message

3. Manual trigger
- Condition: operator executes explicit flush from Team Debug surface.

## Runtime Integration Points

No code is added in this phase; this section defines intended integration points.

- Session output ingestion:
  - detect soft/hard trigger candidates from ACP stream and system stream updates.
- Team manager orchestration:
  - execute flush by `session_id` and resolve `(team_id, run_id, step_id, member_id)`.
- Event pipeline:
  - append lifecycle events into `team_run_events`.

## Flush State Machine

States:

- `idle`
- `started`
- `persisted`
- `noop`
- `failed`

Transitions:

1. `idle -> started`
2. `started -> persisted` when durable payload written
3. `started -> noop` when no new durable payload exists
4. `started -> failed` when flush pipeline errors

Retry policy:

- retry allowed from `failed` in later ticks or manual trigger
- idempotency enforced by checkpoint cursor

## Data Selection Contract

### Selection Unit

- Source: `agent_events` rows for one `(agent_id, session_id)` mapped to Team step.
- Read window: `(last_event_id, latest_event_id]`.

### Checkpoint

Per `(run_id, member_id, session_id)` persist:

- `last_event_id`
- `updated_at`

### Filtering

- keep ACP/system messages relevant to decision trail
- skip redundant noise when no new durable information is detected

## Artifact Storage Contract

### Root

- `<agent_workspace>/.cache/context/run/<run_id>/`

### Naming

- `artifact-<seq>-memory-flush.json`
- `artifact-<seq>-memory-summary.txt`

### JSON Envelope (v1)

```json
{
  "schema_version": 1,
  "team_id": "<team_id>",
  "run_id": "<run_id>",
  "member_id": "<member_id>",
  "session_id": "<session_id>",
  "source_event_range": {
    "from_exclusive": 120,
    "to_inclusive": 188
  },
  "summary_text": "<bounded redacted summary>",
  "observations": [
    {
      "event_id": 153,
      "stream": "acp",
      "type": "agent_message",
      "excerpt": "<bounded redacted excerpt>"
    }
  ],
  "created_at": 1766400000
}
```

## Tiered Flush Output (L1 -> L2)

Memory flush writes to tiered filesystem memory:

- `L1` (mandatory):
  - write run-scoped artifact under `.cache/context/run/<run_id>/...`
  - keep high-fidelity excerpts for replay/debug
- `L2` (optional promotion):
  - write curated durable summary under `.cache/context/memory/*.md`
  - only for cross-run facts/decisions that remain useful after run completion

Promotion is deterministic and explicit; no implicit rewrite of existing `L2` records.

## L2 Promotion Policy (Design)

### Promotion Criteria

A flush batch is eligible for `L2` promotion only when all are true:

1. durability: value expected to survive current run boundary;
2. specificity: concrete and testable fact/decision, not vague narrative;
3. safety: passes redaction policy with no unresolved secret-like tokens.

### Promotion Buckets

- `project_facts.md`
  - validated environment/project facts, stable constraints, contract facts
- `decision_journal.md`
  - accepted decisions with rationale and timestamp
- `open_questions.md`
  - unresolved blocking questions and required confirmation

### Promotion Record Shape

Each promoted entry should include:

- source pointer (`run_id`, `member_id`, `session_id`, `event_range`)
- concise statement (`1-3` lines)
- confidence tag (`high|medium|low`)
- reviewer tag (`auto|human`)
- created_at

## Database Schema Proposal (Design)

### Table: `team_context_artifacts`

Purpose:

- index filesystem artifacts for replay/debug and pointer resolution.

Columns:

- `id INTEGER PRIMARY KEY AUTOINCREMENT`
- `team_id TEXT NOT NULL`
- `run_id TEXT NOT NULL`
- `member_id TEXT NOT NULL`
- `session_id TEXT NOT NULL`
- `artifact_kind TEXT NOT NULL` (`memory_flush`, `summary`, ...)
- `artifact_path TEXT NOT NULL`
- `artifact_size_bytes INTEGER NOT NULL`
- `content_checksum TEXT`
- `event_id_from INTEGER`
- `event_id_to INTEGER`
- `created_at INTEGER NOT NULL`

Indexes:

- `(run_id, member_id, created_at DESC)`
- `(session_id, created_at DESC)`

### Table: `team_context_flush_checkpoint`

Purpose:

- idempotent incremental flush cursor per session scope.

Columns:

- `team_id TEXT NOT NULL`
- `run_id TEXT NOT NULL`
- `member_id TEXT NOT NULL`
- `session_id TEXT NOT NULL`
- `last_event_id INTEGER NOT NULL`
- `updated_at INTEGER NOT NULL`

Primary key:

- `(run_id, member_id, session_id)`

## Event Contract (Design)

### Event Types

- `memory_flush_started`
- `memory_flush_persisted`
- `memory_flush_noop`
- `memory_flush_failed`

### Payload Shape

Required fields:

- `team_id`
- `run_id`
- `step_id` (nullable when manual trigger outside step state)
- `member_id`
- `session_id`
- `trigger` (`soft_threshold` | `hard_error` | `manual`)
- `workspace_root` (sanitized)
- `ts`

Extra fields by event:

- `memory_flush_persisted`:
  - `artifact_pointer`
  - `artifact_size_bytes`
  - `event_id_from`
  - `event_id_to`
- `memory_flush_noop`:
  - `reason` (`no_new_events` | `filtered_empty`)
- `memory_flush_failed`:
  - `reason_code`
  - `error_excerpt`

## Continuity Envelope Integration

On successful flush:

- continuity snapshot may include artifact pointer references.
- subsequent run bootstrap can inject:
  - bounded summary text
  - compact artifact pointers
- full artifact body remains file-backed and optional to load.

## Retention And TTL Policy (Design)

### L1 Retention

- primary control: age-based + size-based retention
- default guidance:
  - retain recent run artifacts for short-horizon replay/debug
  - evict oldest artifacts when workspace budget is exceeded
- eviction must preserve index consistency (no dangling pointers)

### L2 Retention

- durable by default, but compact periodically:
  - merge duplicates
  - archive stale low-confidence notes
  - keep accepted decisions and validated facts
- compaction should be append-only in audit trail (`decisions.md` / events), even if summary files are rewritten

## Governance And Safety (Design)

### Human Override

- allow operator/leader to:
  - approve/reject specific `L2` promotions
  - downgrade mistaken durable entries back to `L1` references
  - annotate corrections without deleting historical provenance

### Data Hygiene

- enforce deterministic redaction marker (`[redacted]`)
- deduplicate near-identical promoted facts by normalized key
- prevent uncontrolled growth by per-file size caps and periodic compaction window

## Redaction Policy

Before summary or artifact write:

- apply key-based secret redaction (`token`, `secret`, `password`, `authorization`, `api_key`, `apikey`).
- normalize redaction marker to deterministic `[redacted]`.
- enforce max chars on summary and excerpt fields.

## Failure Handling

Failure cases and behavior:

1. Session mapping missing
- emit `memory_flush_failed` with `reason_code=session_mapping_missing`
- do not block run progression

2. Filesystem write failure
- emit `memory_flush_failed` with `reason_code=artifact_write_failed`
- keep checkpoint unchanged for retry

3. DB write failure for checkpoint/artifact index
- emit `memory_flush_failed` with `reason_code=db_write_failed`
- attempt best-effort filesystem cleanup is optional in v1

## Observability

Metrics proposal:

- `team_memory_flush_total{status,trigger}`
- `team_memory_flush_artifact_bytes_total`
- `team_memory_flush_noop_total{reason}`
- `team_memory_flush_failed_total{reason_code}`
- `team_memory_flush_latency_ms`

Debug visibility:

- Team Events panel shows all flush events.
- Team Debug panel links artifact pointer and event range.

## Test Plan (Design)

### Unit

- trigger condition evaluation (soft/hard/manual)
- event payload schema validation
- checkpoint progression and idempotency
- redaction and bounded summary behavior

### Integration

- flush by `session_id` resolves Team run/member correctly
- persisted artifact row + checkpoint row + run event are consistent
- noop path emits correct event with no artifact write
- failure path does not alter run/step terminal convergence

### E2E

- repeated Team run with forced budget pressure:
  - flush events visible
  - continuity reuse includes pointer metadata
  - no cross-workspace file writes observed

## Rollout Plan

1. Land schema + manager API + event contract.
2. Land flush executor (manual trigger path first).
3. Add soft/hard automatic trigger integration.
4. Add UI debug affordances and E2E verification.

## Open Questions

- Should v1 checkpoint key include `agent_id` in addition to `(run_id, member_id, session_id)`?
- Should hard trigger parse provider-specific error codes or keep text-based matching only in v1?
- Should flush events include a compact hash of summary text for faster dedupe analysis?
