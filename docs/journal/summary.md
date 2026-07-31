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
| Frontend, UI, and mobile surfaces | `docs/features/frontend-design.md`, `docs/features/workspace-unified-ia.md`, `docs/features/web-static-assets-and-pwa.md` | `*-web-*.md`, `*-frontend-*.md`, `*-mobile-*.md`, `*-ui-*.md`, `*-pwa-*.md` |
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
- The message-store foundation now includes a body-free `cf_index` projection boundary, authority
  repair primitive, and RocksDB `cf_index` tests while keeping SQLite as the normal read path.
- Agent workflow organization now defines project-owned SOP, skill, checklist, testing, and
  observability workflow contracts without copying another project's process taxonomy.
- Team system prompt organization now defines prompt layers, pointer-first runtime tails,
  skill/checklist entry points, and tool-neutral durable knowledge boundaries.
- Access-control planning now defines user roles, capability gates, and route authorization
  guardrails for moving beyond root-only checks.
- Object storage now has an OpenDAL-backed foundation, stable metadata/object-byte boundary,
  owner-scoped Team/task/agent upload API checkpoints, and a MinIO-backed S3-compatible fixture.
- PWA/static asset routing now has a canonical cache-control contract and local app-router coverage
  for shell fallbacks, manifest, service worker, immutable hashed assets, and missing asset paths.
- First-run setup now separates local instance configuration from browser root-account bootstrap:
  `agenthub init` owns runtime role/config, while the web login route owns the initial root account.
- Team adoption extensions now keep stopped-only move, workspace-content copy, and memory/context
  seeding as separate post-copy-first modes with explicit ownership, runtime, and provenance gates.
- Workspace UI compaction wave 2 moved shell-density and channel-first lens rules from four April
  micro-journals into the canonical unified workspace IA spec.
- Team conversation/composer compaction wave 2 moved chat-style composer, visible payload,
  selection resilience, message-row, and rich-text rules into canonical Team/frontend specs.
- Team conversation/composer closeout adds component evidence that the ACP input dock shares the
  same lightweight editor row, actions row, helper text, and send-button language as Team
  channel/thread composers.
- ACP UI compaction wave 2 moved tool-call folding/grouping, humanized payload, markdown safety,
  mobile header, debug-shell, semantic-class, and virtualization rules into canonical ACP/frontend
  specs.
- ACP conversation long-history guard now explicitly covers the pinned recent-tail state and the
  scroll-up full-source virtualization state at the hook boundary.
- ACP conversation row rerender guard memoizes row wrappers so tool-call focus changes rerender only
  rows whose focused state changes.
- Team thread row rerender guard memoizes root/reply rows so composer-local state changes do not
  rebuild unchanged visible thread messages.
- Team thread reply windows keep extremely long reply panes bounded on first render while preserving
  an explicit expansion path for earlier replies.
- Team channel activity row rerender guard memoizes visible channel rows so composer-local state
  changes and polling refreshes skip unchanged row inputs.
- Team channel timelines already render a bounded visible tail while pinned to the bottom and expand
  the full source list after user scroll-up.
- Team mailbox conversation row rerender guard memoizes mailbox message rows so draft changes and
  polling refreshes skip unchanged visible row inputs.
- Team mailbox conversation tail windows keep pinned mailbox chats bounded and keep visible bulk
  actions scoped to the rendered window.
- Team task board card rerender guard memoizes kanban task cards so filter/detail state changes
  skip unchanged visible card inputs.
- Team workspace context rerender split removes the full workbench runtime object from the shell
  context so conversation-only updates do not wake shell consumers.
- Frontend performance browser baseline records an unauthenticated local workspace-shell trace with
  healthy login-shell LCP/CLS plus authenticated Playwright long-history windowing coverage for Team
  channel and Team-member ACP surfaces.
- Team self-maintenance/deferred follow-up closeout records local coverage for profile patch,
  one-shot time triggers, and operator-controlled agent loop behavior.

Start with:

- `2026-07-13-message-body-store-phase1-dual-write.md`
- `2026-07-18-message-index-cf-index-foundation.md`
- `2026-07-19-pwa-cache-control-router-guard.md`
- `2026-07-19-first-run-web-setup-surface.md`
- `2026-07-19-team-adoption-extension-contract.md`
- `2026-07-19-workspace-ui-compaction-wave2.md`
- `2026-07-19-team-conversation-composer-compaction-wave2.md`
- `2026-07-19-team-conversation-composer-closeout.md`
- `2026-07-19-acp-ui-compaction-wave2.md`
- `2026-07-19-acp-conversation-long-history-guard.md`
- `2026-07-19-acp-conversation-row-rerender-guard.md`
- `2026-07-19-team-thread-row-rerender-guard.md`
- `2026-07-19-team-thread-reply-window.md`
- `2026-07-19-team-channel-activity-row-rerender-guard.md`
- `2026-07-19-team-channel-conversation-tail-window.md`
- `2026-07-19-team-mailbox-conversation-row-rerender-guard.md`
- `2026-07-19-team-mailbox-conversation-tail-window.md`
- `2026-07-19-team-task-board-card-rerender-guard.md`
- `2026-07-19-team-workspace-context-rerender-split.md`
- `2026-07-19-frontend-performance-browser-baseline.md`
- `2026-07-19-team-self-maintenance-deferred-followup-closeout.md`
- `2026-07-15-agent-operating-workflows.md`
- `2026-07-15-team-system-prompt-contract.md`
- `2026-07-16-access-control-roles.md`
- `2026-07-16-object-storage-opendal.md`
- `2026-07-18-object-upload-owner-scopes.md`
- `2026-07-18-object-store-s3-minio-fixture.md`

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
