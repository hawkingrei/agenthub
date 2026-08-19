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
- Official release matrices now enable the OpenDAL S3 backend explicitly while keeping the local
  filesystem as the runtime default and artifact/provider evidence as separate gates.
- Server-side download ingestion now records durable terminal outcome, latency, byte, failure-class,
  and compensation-cleanup aggregates without introducing high-cardinality source labels.
- Feature-doc compaction wave 2 now keeps Workspace, Team conversation/composer, and ACP-heavy UI
  contracts in canonical specs while retaining dated journals only for rollout evidence.
- Dependabot remediation now removes all 16 open alert versions from the Rust ACP, web, and
  userdocs dependency graphs while retaining explicit upstream dependency migration follow-ups.
- User documentation now follows current release artifacts and runtime behavior across installation,
  onboarding, agent lifecycle, streaming, API automation, nodes, security, storage, and operations.
- Team prompts now separate an idea's validity from requests to propagate or durably encode it, with
  a compact role boundary and a detailed message-intake judgment gate.
- Message-store `cf_body`/`cf_index` now has executable dual-read comparison and full rebuild/
  backup-restore recovery evidence, narrowing the remaining SQLite-retirement work to the unstarted
  transactional `ControlStore` design for Team/Agent/run/mailbox/permission/idempotency authority.
- The `ControlStore` design is now defined and its Phase 1 foundation (conditional update, idempotent
  insert, and audit primitives, SQLite remaining the authority) is landed. Phase 3 backfill is complete
  for every duplicated CAS/idempotency/audit call site found, including a third, previously-undiscovered
  duplicate matcher; `docs/todo.md`'s SQLite-retirement item is closed, leaving only the ongoing Phase 2
  discipline of new authority code adopting the primitives as it lands.
- PWA install icons and the browser-tab favicon were blank/missing (a placeholder square with no logo,
  and no favicon link at all); a follow-up fix then discovered the "real brand asset" used to fill them
  in was actually the third-party Slock OAuth provider's icon, not AgentHub's own, so every installed
  instance displayed another product's logo -- now a neutral "A" monogram placeholder pending a real
  designed mark. Both fixes are unrelated to and unblocked by the still-open deployed-domain PWA
  verification item.
- A code-only UI/UX review surfaced 17 findings against `features/frontend-design.md`'s contracts; the
  six highest-priority ones landed (error hidden behind its own modal, raw status strings leaking past an
  existing humanizer, hand-rolled Team dialogs missing focus trap/return, destructive actions with no
  confirmation, empty admin lists rendering nothing, no loading guard on the join/register flow),
  remainder tracked as a new Frontend UI/UX `todo.md` item.
- A code-only Rust backend correctness review found `safe_paths` never actually restricted an agent's
  `workdir` -- only skill-file loading checked it -- letting any `AgentsManage`-capable account bypass
  the operator-configured allowlist entirely; now enforced (empty allowlist stays permissive, matching
  every existing deployment) for both direct agent creation and Team workspace-copy adoption. The same
  review's `TeamRemoteRelayAdapter.grpc_client_cache` finding turned out worse than an admin-bounded
  leak: the common relay path mints a fresh, timestamp-embedded access token per message, so the cache
  key is unique almost every call and every delivery leaked a live gRPC `Channel` forever; now bounded
  by TTL-based lazy eviction, which also closes a second gap where callers with stable tokens could keep
  serving an expired bearer token from a long-lived cache entry. The internal gRPC bootstrap-token
  comparison (the endpoint that mints cluster-bootstrap credentials for new nodes) used a plain `!=`
  that short-circuits at the first mismatching byte, a timing side-channel; now a constant-time XOR-fold
  comparison. Two other findings from the same review (a remote-relay panic-poisoning trap, a panic
  landmine from an unvalidated task context shape) remain open in the Backend Correctness `todo.md` item.
- The Dependabot `jsdom 29 -> 30` bump failed CI's `Web` job on Node 20 with a cryptic `undici`
  `webidl.util.markAsUncloneable is not a function` -- jsdom 30 explicitly requires Node >=22, and the
  `Web`/`Web E2E`/`Web E2E Mobile` jobs were still pinned to Node 20 (`userdocs.yml`'s unrelated
  Docusaurus build stays on Node 20, out of scope). Bumped those three jobs to Node 22.
- A code-only review of the Team subsystem (Task/Run/Step lifecycle, goal-lease/fork concurrency,
  mailbox, remote relay, permission review, actor protocol) found `task_updates.rs`'s `team_tasks`
  writes had no optimistic-concurrency guard, `release_task_goal_in_tx` released whatever lease was
  active rather than the specific generation observed, and `claim_task_goal_in_tx`/
  `claim_execution_entity` decided claimability in Rust from a stale read but wrote unconditionally --
  all three now guarded via the existing `ControlStore` idiom. Fixing this surfaced a real, separate bug:
  writing `team_tasks.updated_at` from a plain second-granularity `now()` let two same-second writes
  collide and silently defeat the new CAS guard; every writer of that column now writes
  `MAX(updated_at + 1, now())` so it's strictly monotonic regardless of which function touches it. The
  same review also found permission-review's "current reviewer" was resolved two different ways --
  idle-aware at dispatch time, idle-*unaware* (always the first candidate) when re-derived at approval
  time because the persisted target hadn't landed yet -- letting the wrong actor approve/deny a review
  it never received; the approval-time fallback is now removed entirely (fail closed on no persisted
  target) rather than trying to duplicate dispatch's idle-check machinery. `update_task`'s gRPC
  `context_json` (`Replace` patch) only validated it was *valid* JSON, unlike its `context_merge_json`
  sibling which checks the shape -- a non-object value stored this way panicked the next unrelated
  run-status-changing request; now rejected at the RPC boundary, and the consuming
  `run_task_status_sync.rs` code self-heals instead of panicking as defense-in-depth for any
  already-corrupted row. A caller could also set `payload.requires_user_visible_reply: false` on a
  human-to-agent mailbox message to silently disable the system's reply-obligation tracking for it, since
  normalization only backfilled the field when absent; the human-to-agent case is now forced regardless
  of payload, derived from `from_actor_id` (not the payload's own spoofable `source_kind`) so it can't be
  defeated by also faking the source. Reply-obligation "credit" matching also keyed only on
  `(agent_actor_id, human_actor_id)`, ignoring which thread/conversation a message belonged to, so a
  reply in one conversation could incorrectly close an unrelated open obligation in a different one;
  fixed with a two-tier key (exact thread/conversation scope first, untagged-reply loose-pool fallback)
  so a reply that declares a thread can never satisfy an obligation in a different one, while plain
  untagged replies keep working as before. Other findings from the same review round are tracked in the
  Backend Correctness `todo.md` item.
- The same review found `reassign_reply_required_message` (transfer/escalate/takeover) had no CAS guard
  on its source-message `UPDATE`, unlike `triage_message_impl`, and inserted the reassigned message with
  `idempotency_key = NULL`. Added the guard plus a stable idempotency key with an `INSERT OR IGNORE` +
  fetch-existing fallback. A genuine-concurrency soak test (new WAL-mode `setup_concurrent_mailbox_db`
  helper) found SQLite's WAL snapshot-conflict detection already prevents literal duplicate rows in this
  stack -- the CAS guard is kept as defense-in-depth and for a correct `Conflict` error rather than a raw
  database error, not as the sole mechanism; the idempotency key is the part with directly fix-sensitive
  test coverage (client-retry duplicate submission).
- The same review's last finding: `message_index_projection.rs`'s repair passes silently coerced
  unparseable `payload_json`/`input_json` to `Value::Null` with no logging, so genuinely corrupt rows
  indexed as empty messages with no trace. Kept the `Value::Null` fallback (aborting the whole repair
  batch on one bad row would be worse) but added a structured `tracing::warn!` (source table, row id,
  field, parse error) via a new shared `parse_projection_json` helper replacing four inline duplicates.
  This closes out all seven findings from the 2026-08-17 Team-subsystem review round.
- `safe_paths` -- the admin-configured workdir allowlist, including the enforcement landed earlier this
  month -- has been removed entirely rather than fixed: a startup-seeding bug meant the "empty allowlist
  = permissive" design intent was never actually reachable once a server had booted once, and the
  decision was to drop the feature (workdirs are no longer restricted at all) instead of patching the
  seed or widening the default. Removal spans the config field/DB table/migration, the admin API and UI,
  and all three independent consumers (agent-creation/Team-adoption enforcement, the ACP skill-loading
  allowlist which is now fully unrestricted, and a team-runtime repo-inference heuristic which is deleted
  outright rather than repointed).

Start with:

- `2026-08-01-team-conversation-style-convergence.md`
- `2026-08-06-teamspace-control-plane.md`
- `2026-08-07-team-goal-lease-foundation.md`
- `2026-08-08-object-store-s3-release-enablement.md`
- `2026-08-08-object-storage-download-observability.md`
- `2026-08-09-feature-docs-compaction-wave2-closeout.md`
- `2026-08-12-dependabot-security-remediation.md`
- `2026-08-13-user-documentation-release-readiness.md`
- `2026-08-16-pwa-icon-branding-fix.md`
- `2026-08-16-safe-paths-workdir-enforcement.md`
- `2026-08-16-grpc-relay-client-cache-ttl.md`
- `2026-08-16-bootstrap-token-constant-time-compare.md`
- `2026-08-16-frontend-uiux-review-round1-fixes.md`
- `2026-08-17-pwa-icon-borrowed-slock-mark-fix.md`
- `2026-08-14-team-idea-propagation-judgment.md`
- `2026-08-17-ci-web-node22-for-jsdom30.md`
- `2026-08-17-goal-lease-cas-hardening.md`
- `2026-08-17-permission-review-reviewer-target-consistency.md`
- `2026-08-17-task-context-json-shape-validation.md`
- `2026-08-17-reply-obligation-client-suppression-fix.md`
- `2026-08-17-reply-obligation-thread-scoped-matching.md`
- `2026-08-17-mailbox-reassignment-cas-and-idempotency.md`
- `2026-08-17-message-index-repair-corruption-visibility.md`
- `2026-08-19-safe-paths-removal.md`

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
