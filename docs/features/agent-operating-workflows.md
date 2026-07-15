# Agent Operating Workflows

## Problem

Agent-facing instructions need a place for repeatable engineering workflows without turning
`AGENTS.md` or runtime prompts into large manuals. The repository already has feature specs,
journals, TODO, and local skills; what was missing is a project-owned rule for when to use each
surface.

## Scope

- Repository-level workflow organization for coding agents.
- The boundary between `AGENTS.md`, feature specs, journals, TODO, and `.agents/skills`.
- Testing and observability workflows that agents repeat during review, CI, runtime diagnosis, and
  release work.
- Tool-neutral handling of long-lived agent knowledge.

## Non-Goals

- Defining platform system prompts.
- Copying another project's workflow taxonomy or business processes.
- Requiring a private memory backend for contributors.
- Moving all existing feature specs into a new directory tree.

## Architecture

Agent operating knowledge uses these repository-owned surfaces:

| Surface | Owns | Does not own |
| --- | --- | --- |
| `AGENTS.md` | Short project charter, hard engineering rules, canonical entry points. | Long procedures, product manuals, personal memory. |
| `docs/features/` | Stable contracts for product, runtime, API, testing, observability, and operations. | Chronological implementation logs. |
| `docs/journal/` | Dated implementation records and validation evidence. | New normative contracts not promoted to a feature spec. |
| `docs/todo.md` | Active remaining work only. | Completed history or long design notes. |
| `.agents/skills/` | Executable, task-triggered procedures for local coding agents. | Broad product knowledge or human-facing documentation. |
| External searchable memory/knowledge system | Durable long-lived knowledge that should not become an open-source repository contract. | Required project rules for contributors. |

## Contracts

### 1) Workflow Type Contract

Use project-owned workflow types by weight:

- `SOP`
  - Stable, reviewable engineering procedure.
  - Belongs in `docs/features/` when it defines a durable contract, or in a dedicated docs page when
    it is operational but not product behavior.
  - Must name scope, owner surface, required evidence, and failure handling.
- `skill`
  - Local executable procedure for an agent.
  - Belongs under `.agents/skills/<name>/SKILL.md`.
  - Must include trigger conditions, decision boundary, steps, validation, and fallback behavior.
- `checklist`
  - Lightweight repeated workflow that is not yet worth a skill.
  - Belongs in the relevant feature spec, journal follow-up, or TODO item.
  - Should be promoted to a skill only after repeated use shows the steps are stable.

Do not introduce workflow categories named after another project. If a pattern is useful, rewrite it
as an AgentHub-specific SOP, skill, or checklist with this repository's commands and evidence.

### 2) Feature Spec Categories

Use these categories when locating or compacting specs:

| Category | Purpose | Current anchors |
| --- | --- | --- |
| Product and workspace surfaces | User-facing Team, agent, workspace, and UI contracts. | `agents-teams.md`, `team-channels-threads.md`, `workspace-unified-ia.md`, `frontend-design.md` |
| Runtime and provider control | ACP, provider sessions, subprocess behavior, runtime prompt delivery. | `acp-runtime.md`, `backend-runtime-logic.md`, `agent-runtime-profiles.md` |
| Team execution and coordination | Task ownership, mailbox, actor CLI, collaboration behavior. | `team-execution-vocabulary.md`, `team-mailbox-intake-and-ownership.md`, `actor-foundation.md`, `teams-collaboration-playbook.md` |
| Distributed and node operation | Nodes, node registry, internal transport, multi-node execution. | `agent-nodes.md`, `distributed-node-architecture.md`, `distributed-node-registry-and-gossip.md` |
| Storage and message authority | Durable message state, metadata, archive, body tiering, migration safety. | `logical-message-metadata-contract.md`, `message-archive-lancedb.md`, `message-storage-tiering.md` |
| Testing and quality guardrails | Regression evidence, fixture consistency, protected objects. | `test-regression-guardrails.md` |
| Observability and operations | Debugging, profiling, diagnostic evidence, release/install operation. | `runtime-diagnostics.md`, `pyroscope-profiling.md`, `npm-binary-distribution.md`, `debian-systemd-distribution.md` |
| Integrations | Linkers, OAuth/linker boundaries, external runtime integration. | `app-linkers.md`, `slock-oauth-linkers.md` |

### 3) Testing SOP Contract

Testing workflow starts by naming the behavior at risk:

1. Protected object
   - Authority row, state transition, route target, permission boundary, UI behavior, or artifact.
2. Invariant
   - The property that must survive future edits.
3. Terminal oracle
   - Persisted state, visible UI behavior, emitted event, diagnostic classification, or release
     artifact.
4. Boundary proof
   - Failing boundary plus closest valid neighbor.
   - Persistence fixtures follow `test-regression-guardrails.md`.
5. Validation layer
   - Unit, contract, integration, browser/e2e, CI check, or manual acceptance surface.

### 4) Observability SOP Contract

Observability workflow starts from the acceptance surface and works backward:

1. Name the acceptance surface.
   - Browser state, API/CLI response, persisted event, release artifact, profile, trace, or CI job.
2. Capture minimal reproducible evidence.
   - Command, run id, job URL, trace bundle, log snippet, diagnostic JSON, screenshot, or artifact id.
3. Classify the failure layer.
   - Runtime/session, provider prompt, permission/tool gate, persistence, SSE/broadcaster, frontend
     render/cache, CI environment, or release packaging.
4. Use the narrow diagnostic surface first.
   - Runtime diagnosis starts from `runtime-diagnostics.md`.
   - CPU diagnosis starts from `pyroscope-profiling.md`.
   - CI diagnosis starts from exact check/job evidence.
5. Report proof boundaries.
   - Say what the evidence proves, what it does not prove, and which acceptance surface remains
     unverified.

### 5) Initial Project-Owned Workflow Candidates

Promote these into skills or checklists as they next need maintenance:

- PR review follow-up and thread resolution.
- CI failure triage and rerun evidence.
- Protected-object test evidence.
- Runtime stuck diagnosis.
- Release artifact verification.
- Prompt or skill update review.

## Validation Matrix

| Change | Required validation |
| --- | --- |
| Edit `AGENTS.md` workflow rules | `git diff --check`; confirm wording stays tool-neutral. |
| Edit Team runtime prompt templates | `cargo test -p agenthub-team-prompts -- --nocapture`. |
| Add a skill | Validate skill frontmatter and run any referenced focused check. |
| Add or revise testing SOP guidance | Keep it consistent with `test-regression-guardrails.md`. |
| Add or revise observability SOP guidance | Keep it consistent with `runtime-diagnostics.md` and `pyroscope-profiling.md`. |

## Operational Notes

- Prefer a checklist before adding a new skill when the workflow has not repeated enough to stabilize.
- Prefer a feature spec before adding prompt prose when the workflow is a durable contract.
- Prefer external memory before adding repository docs when the knowledge is personal, cross-tool, or
  not required for open-source contributors.
- Keep project-owned skills small and command-oriented.

## Open Risks

- Runtime prompts are already long; future workflow extraction should reduce prompt text rather than
  add more.
- Feature categories can drift as new domains appear; update this spec and `docs/features/README.md`
  together.
- Some workflow candidates may need code support before they can become useful skills.

## Source Journals

- [2026-07-15 Agent Operating Workflows](../journal/2026-07-15-agent-operating-workflows.md)
