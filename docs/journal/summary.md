# Journal Summary

This file is the index for `docs/journal/`.

Use it to find dated implementation checkpoints, rollout notes, validation evidence, and historical
decision context without scanning every dated file. Stable contracts still belong in
`docs/features/`; active remaining work belongs in `docs/todo.md`.

## How To Use This Index

- Search by date when reconstructing a rollout timeline.
- Search by topic prefix when investigating a subsystem.
- Start from the canonical feature spec when the question is about current behavior, then use the
  journal entries for implementation history and validation evidence.
- Add new entries as `YYYY-MM-DD-topic.md`; keep one topic per file.
- Prefer updating this summary when many small journals converge into one durable topic.

Useful commands:

```bash
find docs/journal -maxdepth 1 -type f -name '2026-06-*.md' | sort
find docs/journal -maxdepth 1 -type f -name '*team-mailbox*.md' | sort
rg -n "Validation|Follow-Ups|Key Decisions" docs/journal/2026-06-*.md
```

## Topic Map

| Topic | Canonical Docs | Journal File Patterns |
| --- | --- | --- |
| ACP runtime and provider integration | `docs/features/acp-runtime.md` | `*-acp-*.md`, `*-codex-acp-*.md`, `*-claude-acp-*.md` |
| Team collaboration, channels, and tasks | `docs/features/agents-teams.md`, `docs/features/team-channels-threads.md`, `docs/features/team-execution-vocabulary.md` | `*-team-*.md`, `*-teams-*.md` |
| Team mailbox and actor runtime | `docs/features/team-mailbox-intake-and-ownership.md`, `docs/features/actor-foundation.md` | `*-team-mailbox-*.md`, `*-actor-*.md` |
| Workspace memory and continuity | `docs/features/team-workspace-memory-contract.md`, `docs/features/workspace-unified-ia.md` | `*-workspace-*.md`, `*-context-*.md`, `*-memory-*.md` |
| Frontend, UI, and mobile surfaces | `docs/features/frontend-design.md`, `docs/features/workspace-unified-ia.md` | `*-web-*.md`, `*-frontend-*.md`, `*-mobile-*.md`, `*-ui-*.md` |
| Agent nodes and distributed execution | `docs/features/agent-nodes.md`, `docs/features/distributed-node-architecture.md` | `*-node-*.md`, `*-distributed-*.md`, `*-p2p-*.md` |
| Release, packaging, and install paths | `docs/features/npm-binary-distribution.md`, `docs/features/debian-systemd-distribution.md` | `*-release-*.md`, `*-npm-*.md`, `*-debian-*.md`, `*-homebrew-*.md` |
| CI, Bazel, coverage, and dependencies | `docs/developer-setup.md` | `*-ci-*.md`, `*-bazel-*.md`, `*-codecov-*.md`, `*-dependabot-*.md` |
| Access control, auth, and permissions | `docs/features/access-control-and-roles.md`, `docs/features/acp-runtime.md` | `*-auth-*.md`, `*-access-control-*.md`, `*-permission-*.md` |
| Message archive, metadata, and object storage | `docs/features/logical-message-metadata-contract.md`, `docs/features/message-archive-lancedb.md`, `docs/features/object-storage-opendal.md` | `*-message-*.md`, `*-metadata-*.md`, `*-lancedb-*.md`, `*-object-storage-*.md` |
| RARA and app integrations | `docs/features/rara-direct-integration.md`, `docs/features/app-linkers.md` | `*-rara-*.md`, `*-app-*.md`, `*-linker-*.md` |

## Monthly Rollups

### 2026-02

Main shape:

- ACP output, chunking, permission, and provider bootstrap foundations.
- Team A2A, run lifecycle, mailbox, context memory, and shared status groundwork.
- Bazel, Rust toolchain, proto generation, coverage, and CI baseline setup.
- Frontend migration toward Mantine, Tailwind, mobile stability, and output rendering hardening.

Start with:

- `2026-02-07-acp-output-cache-and-polling.md`
- `2026-02-12-a2a-agent-team-phase1.md`
- `2026-02-15-bazel-native-rules-rust-core.md`
- `2026-02-20-web-tailwind-ui-phase1-auth-team-sidebar.md`
- `2026-02-24-team-operating-model-spec.md`

### 2026-03

Main shape:

- Team runtime, actor CLI, inbox/mailbox, shared-thread, and Kanban behavior matured.
- Internal gRPC, P2P, node, and distributed execution work started to separate runtime boundaries.
- Team UI shell, workbench layout, route splitting, mobile polish, and permission-review routing were
  repeatedly hardened.
- Codex ACP syncs, dependency upgrades, CI regressions, and review follow-ups drove stabilization.

Start with:

- `2026-03-05-team-conversation-event-bus-contract.md`
- `2026-03-19-team-task-ownership-contract.md`
- `2026-03-22-actor-runtime-cli-first.md`
- `2026-03-28-internal-p2p-core-crate.md`
- `2026-03-31-agents-api-explicit-validation.md`

### 2026-04

Main shape:

- Workspace unified IA, Team page split, runtime context layout, and Notion-density UI direction
  became the active product surface.
- Channel/thread backend primitives, thread pane work, task panels, and conversation selection
  resilience moved Team collaboration toward topic-first navigation.
- Node join, release assets, OpenSSL/static build behavior, and Homebrew/install docs expanded the
  distribution path.
- Context compaction, prompt tail slimming, and memory-index work clarified continuity boundaries.

Start with:

- `2026-04-05-team-prompt-first-principles.md`
- `2026-04-10-team-workspace-memory-contract.md`
- `2026-04-18-workspace-unified-ia-spec.md`
- `2026-04-19-team-channel-thread-backend-primitives.md`
- `2026-04-26-node-startup-boundary-verification.md`

### 2026-05

Main shape:

- Message metadata, group id projection, channel/thread event correlation, and archive foundations
  were consolidated.
- Team task-first, mailbox phase 2/3, inbound envelope normalization, transfer/takeover, and
  permission-review routing became the main collaboration hardening line.
- Codex upgrades and prompt operating contract refreshes kept ACP provider behavior current.
- Release P0 validation, npm publishing, shared primitives, and route cleanup tightened the product
  delivery path.

Start with:

- `2026-05-04-lancedb-message-archive-phase1.md`
- `2026-05-06-message-archive-step-lifecycle-run-events.md`
- `2026-05-20-team-mailbox-phase2-ownership-and-task-links.md`
- `2026-05-27-team-task-first-p0-verification.md`
- `2026-05-31-team-mailbox-inbound-envelope-normalization.md`

### 2026-06

Main shape:

- Team node continuity, remote node posture, and RARA team-mode requirements captured the current
  multi-agent/runtime direction.
- Codex 136/138, generic Codex ACP entrypoint, Claude ACP support, and ACP TODO closeout updated the
  provider/runtime surface.
- Message store foundation, Pyroscope bootstrap, Rust 1.96 baseline, and Debian systemd packaging
  moved infrastructure and release readiness forward.

Start with:

- `2026-06-02-team-mailbox-phase3-closeout.md`
- `2026-06-04-team-node-continuity-rollup.md`
- `2026-06-08-rara-team-modes-requirements.md`
- `2026-06-10-message-store-foundation-crate.md`
- `2026-06-13-debian-systemd-release-package.md`

### 2026-07

Main shape:

- Phase 1 message-body storage now keeps SQLite compatibility bodies while asynchronously staging
  compressed RocksDB copies through a durable outbox and checkpointed backfill.
- Agent workflow organization now defines project-owned SOP, skill, checklist, testing, and
  observability workflow contracts without copying another project's process taxonomy.
- Team system prompt organization now defines prompt layers, pointer-first runtime tails,
  skill/checklist entry points, and tool-neutral durable knowledge boundaries.
- Access-control planning now defines user roles, capability gates, and route authorization
  guardrails for moving beyond root-only checks.
- Object storage now has an OpenDAL-backed foundation, stable metadata/object-byte boundary,
  owner-scoped Team/task/agent upload API checkpoints, and a MinIO-backed S3-compatible fixture.
- Codecov project coverage gating now tolerates small multi-flag aggregation movement while keeping
  patch coverage and upload fail-fast checks strict.

Start with:

- `2026-07-13-message-body-store-phase1-dual-write.md`
- `2026-07-15-agent-operating-workflows.md`
- `2026-07-15-team-system-prompt-contract.md`
- `2026-07-16-access-control-roles.md`
- `2026-07-16-object-storage-opendal.md`
- `2026-07-18-object-upload-owner-scopes.md`
- `2026-07-18-object-store-s3-minio-fixture.md`
- `2026-07-20-codecov-project-threshold.md`
- `2026-07-22-access-control-root-only-closeout.md`
- `2026-07-22-first-run-setup-closeout.md`
- `2026-07-22-object-storage-download-ingest.md`
- `2026-07-23-object-storage-download-ingest-implementation.md`
- `2026-07-28-codex-145-upgrade.md`

### 2026-08

Main shape:

- Conversation surfaces now share neutral message-bubble styling while preserving their own
  interaction semantics; final deployed-browser evidence remains open.
- Teamspace now has invite-only local-account membership, auditable revocation, single-owner
  execution claims, and explicit handoff control-plane foundations.
- Task claims now reserve durable, generation-fenced Team and member goal capacity; terminal
  transitions and handoff release the reservation while preserving audit history.
- Read-only forks are Team-bounded, parent-generation-fenced, exposed through authorized APIs, and
  return immutable results to the parent Task evidence stream.

Start with:

- `2026-08-01-team-conversation-style-convergence.md`
- `2026-08-06-teamspace-control-plane.md`
- `2026-08-07-team-goal-lease-foundation.md`

## Compaction Rules

- Keep original dated journals when they contain validation evidence, PR context, or detailed
  implementation chronology.
- Compact by promoting stable conclusions into `docs/features/` and linking this summary to the
  canonical doc.
- Do not turn this file into a complete file listing; prefer monthly rollups and topic patterns.
- Remove or rewrite old journal prose only when it is stale, contradictory, and superseded by a
  canonical feature spec.

## Maintenance

- Keep this file as a navigational index, not a changelog.
- Prefer topic patterns and canonical-doc pointers over listing every journal file.
- Update this index when a new major journal category appears or when a canonical feature spec
  replaces older scattered journal guidance.
