# Backend Runtime Logic Specification

## Problem

Backend Team behavior (authorization, run lifecycle, memory flush, transactional cleanup, and startup semantics)
was captured in multiple point-fix notes. A consolidated backend logic spec is required to keep runtime invariants clear.

## Scope

- Team run/step lifecycle contracts.
- Run-scoped API ownership and authorization boundaries.
- Context flush integration and memory-related runtime behavior.
- Team deletion transactional guarantees.
- Startup runtime policy for Team run recovery.
- Conversation/event outbox persistence-first delivery baseline.

## Non-Goals

- UI behavior details.
- Prompt wording and skill text maintenance.
- Full infra-level deployment architecture.

## Architecture

### 1) Team Manager As Domain Gate

`TeamManager` owns Team runtime state transitions and persistence for:

- run records and step records
- run events
- conversation/task persistence
- mailbox operations
- context flush lifecycle

### 2) Run-Scoped Access Enforcement

Run APIs must enforce owner/team boundaries before mutation:

- load run and team under authenticated user scope;
- reject cross-team run access even if run ID exists;
- keep ownership checks centralized in helper path.

### 3) Context Flush Path

- Manual flush endpoint triggers run-scoped flush pipeline.
- Flush emits lifecycle events (`started`, `persisted`, `noop`, `failed`).
- Oversized payloads are pointerized into workspace artifacts.

### 4) Startup Policy

- Runtime startup must not silently auto-resume previously active Team runs.
- Active runs from prior process (`submitted|working|input_required`) are canceled on startup,
  requiring explicit manual start/restart.

### 5) Team Delete Transaction Boundary

Team delete must execute member-runtime cleanup and Team-domain cascade within one DB transaction
to avoid half-cleanup states and FK-related `500` errors.

### 6) Conversation Event Outbox Baseline

- Conversation/chat events should persist in DB first, then enqueue outbox records for async bus publish.
- Event bus publish failures should not roll back already committed authoritative records.
- Relay workers are responsible for retry/backoff and at-least-once delivery to realtime carriers.

## Contracts

### 1) Run/Step State Transitions

- Run and step states use canonical enum values from `agenthub-team-domain`.
- Terminal states must be explicit (`completed|failed|canceled`).

### 2) API Error Contract

- Ownership and state conflicts should map to explicit 4xx responses.
- Internal persistence/runtime failures should remain observable via structured error chains.

### 3) Transactionality Contract

- Team deletion cleanup order must respect FK dependencies.
- Cleanup and delete operations must share one transaction boundary.

### 4) Shared Utility Contract

- Shared text truncation and similar cross-module helpers should live in shared crates,
  avoiding repeated ad-hoc implementations.

## Validation Matrix

- `cargo check`
- `cargo test -p agenthub -- team::manager::tests`
- `cargo test teams_api_delete_team_cascades_related_run_data -- --nocapture`
- `cargo test teams_router_delete_team_cleans_member_session_dependents_without_500 -- --nocapture`
- `cargo test team_runs_api_enforces_team_owner_access`
- `cargo test flush_run_context_`
- `cargo test team_runs_api_supports_manual_context_flush`
- `cargo test team_runs_api_rejects_invalid_context_flush_trigger`
- `cargo test -p agenthub-text`

## Operational Notes

- Preserve explicit run ownership checks as default, not optional hardening.
- Keep startup behavior deterministic and operator-driven after process restart.
- Keep event trails for flush/recovery paths for postmortem/debug use.

## Open Risks

- Some continuity/compaction flows are still staged (pre-compaction auto flush and long-horizon memory policy).
- Cross-module regression risk remains when run/step semantics change without synchronized tests.

## Source Journals

- `docs/journal/2026-02-23-team-run-access-and-memory-flush-hardening.md`
- `docs/journal/2026-02-23-team-delete-member-session-fk-hardening.md`
- `docs/journal/2026-02-21-api-teams-error-mapping-module-split.md`
- `docs/journal/2026-02-23-backend-error-chain-and-agent-instrumentation.md`
- `docs/journal/2026-02-23-backend-db-agent-error-logging-hardening.md`
