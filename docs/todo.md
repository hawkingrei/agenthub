# TODO

Active backlog only. Keep this file small and current.

## Release And Packaging

- [ ] `P1` Verify preview release partial-asset behavior: semver release run `29194967848` for `v0.0.11` proves Linux `x86_64` / `aarch64` release builds used `release-vendored-openssl`, avoided the stale cross-sysroot OpenSSL panic, and published canonical `agenthub` / `agenthub-acp` assets plus Debian packages. Remaining evidence is a preview release run showing successful binary assets publish when one release matrix target fails. Record the preview workflow run ID and release URL in [journal/2026-04-20-release-vendored-openssl-and-partial-assets.md](journal/2026-04-20-release-vendored-openssl-and-partial-assets.md).

## Team Workspace Browser Matrix

Stable contracts:

- [features/frontend-design.md](features/frontend-design.md)
- [features/workspace-unified-ia.md](features/workspace-unified-ia.md)
- [features/team-channels-threads.md](features/team-channels-threads.md)
- [features/agents-teams.md](features/agents-teams.md)
- [features/teams-collaboration-playbook.md](features/teams-collaboration-playbook.md)

Matrix to keep current on `agenthub.hawkingrei.com` and in PR browser evidence:

- [ ] `P1` Conversation and composer polish: channel/thread composers and the ACP input dock share the same lightweight input language; ACP, thread, and mailbox bubbles share neutral base styling while retaining their local layout and interaction behavior. Remaining work is authenticated deployed-browser evidence for the full workspace flow. Existing notes: [journal/2026-04-24-team-conversation-slock-polish.md](journal/2026-04-24-team-conversation-slock-polish.md), [journal/2026-08-01-team-conversation-style-convergence.md](journal/2026-08-01-team-conversation-style-convergence.md).
- [ ] `P2` PWA installability: local app-router coverage now proves shell fallbacks, `sw.js`, and `manifest.webmanifest` stay `no-cache`, hashed `/assets/*` stay immutable, and missing asset paths do not fall back to HTML. 2026-07-19 production-domain retries at 07:37 UTC, 09:47 UTC, 11:15 UTC, 12:43 UTC, 13:04 UTC, 13:23 UTC, 13:44 UTC, and 14:12 UTC, plus a 2026-07-20 20:08 UTC retry, for `/workspace/teams`, `/sw.js`, `/manifest.webmanifest`, and a missing `/assets/*` probe still returned Cloudflare `502`, which only proves the deployed entrypoint was unavailable. Remaining work is deployed-domain verification that manifest/service-worker remain installable without stale shell caching and that CDN/proxy headers preserve the same contract after the entrypoint is healthy. Stable contract: [features/web-static-assets-and-pwa.md](features/web-static-assets-and-pwa.md); notes: [journal/2026-04-03-pwa-install-and-team-permission-card-collapse.md](journal/2026-04-03-pwa-install-and-team-permission-card-collapse.md), [journal/2026-07-19-pwa-cache-control-router-guard.md](journal/2026-07-19-pwa-cache-control-router-guard.md).

## Team Workspace Architecture

- [ ] `P1` Implement goal/fork control: persist task-backed goal leases, bounded read-only forks, conflict escalation, and transactional Team/member concurrency budgets. Forks must never write workspaces or perform external mutations; informational requests must not preempt an active goal. Stable contract: [features/team-goal-fork-control.md](features/team-goal-fork-control.md).
- [ ] `P1` Phase 1/2 unified workspace shell follow-up: PR #891 merged the Team route facade, shared-lens cleanup, channel-scoped member profile routes, compatibility-only legacy query handling, shared workspace shell primitives, and three-zone Team workbench composition without collapsing Team task-first semantics or turning `thread` into a top-level lens. Desktop task-detail preview now reuses the shared split-pane primitive instead of staying modal-only. Remaining work is deeper shell reuse for other persistent Team context docks and deployed browser evidence across Team navigation surfaces. Stable contract: [features/workspace-unified-ia.md](features/workspace-unified-ia.md); notes: [journal/2026-04-18-workspace-shell-route-phase1.md](journal/2026-04-18-workspace-shell-route-phase1.md), [journal/2026-04-18-workspace-shell-phases-1-3-convergence.md](journal/2026-04-18-workspace-shell-phases-1-3-convergence.md), and [journal/2026-07-20-workspace-task-detail-triad.md](journal/2026-07-20-workspace-task-detail-triad.md).
- [ ] `P1` Complete explicit Team adoption extensions: PR #992 added stopped-only `move existing agent to Team` beside configuration-only copy. Remaining work is opt-in workspace-content copy and memory/context seeding with provenance, exclusion manifests, idempotent retry, focused backend/web tests, and browser evidence. Do not copy sessions, credentials, caches, or mutable source context by default. Stable contract: [features/team-agent-adoption.md](features/team-agent-adoption.md); notes: [journal/2026-05-03-team-agent-adoption-contract.md](journal/2026-05-03-team-agent-adoption-contract.md).
- [ ] `P2` Frontend performance hardening for Team and ACP-heavy pages: reduce avoidable rerenders, keep long lists and live surfaces responsive, and evaluate virtualization/stick-to-bottom behavior for extremely long histories. Stable contracts: [features/frontend-design.md](features/frontend-design.md) and [features/acp-runtime.md](features/acp-runtime.md).

## Team Runtime And Task Model

Stable contracts:

- [features/agents-teams.md](features/agents-teams.md)
- [features/team-execution-vocabulary.md](features/team-execution-vocabulary.md)
- [features/team-workspace-memory-contract.md](features/team-workspace-memory-contract.md)

- [ ] `P1` Verify remote Team direct-mailbox routing on real multi-node teams: after the local API regression and routing fix in [journal/2026-05-26-team-remote-direct-mailbox-routing.md](journal/2026-05-26-team-remote-direct-mailbox-routing.md), confirm direct single-member delivery still preserves mention metadata plus summary/`detail_ref` payloads when the recipient agent is remote and transport falls back to p2p relay in a real multi-node rollout. Existing notes: [journal/2026-03-26-team-direct-mailbox-summary-first.md](journal/2026-03-26-team-direct-mailbox-summary-first.md).
- [ ] `P2` Verify Team agent self-maintenance and deferred follow-up flows: `profile_patch_proposal`, `agent_time_trigger_*`, and operator-controlled `agent_loop` should behave consistently without blocking normal task progress.

## Message Storage

- [x] `P1` Complete the RocksDB `cf_index` authority-derived repair path, per [features/message-storage-tiering.md](features/message-storage-tiering.md). SQLite authority rows now rebuild the conversation, actor mailbox, run-event, and agent-event projections; guarded ordered reads compare high-water marks and exact SQLite page IDs, fall back on any gap, and queue a startup worker for asynchronous repair. Orphan/prune helpers remain diagnostics and explicit maintenance only. Keep normal SQLite bodies readable until a later authority-cutover decision. Notes: [journal/2026-06-10-message-store-foundation-crate.md](journal/2026-06-10-message-store-foundation-crate.md).
- [ ] `P1` Stage SQLite retirement by responsibility, not by a flag flip: Phase 1 `cf_body` dual-write, durable SQLite outbox, startup drainer, and SQLite compatibility reads are landed; `cf_index` is now a rebuildable, guarded delivery projection. Before moving Team, Agent, run, mailbox, permission, or idempotency authority, complete dual-read comparison and full rebuild/backup-restore recovery evidence, then define a transactional `ControlStore` replacement with conditional updates, uniqueness, audit, and per-entity rollback. RocksDB indexes and LanceDB archives must not become control-plane authority by implication. Stable contract: [features/message-storage-tiering.md](features/message-storage-tiering.md).

## Object Storage

- [ ] `P1` Keep `agenthub-object-store/s3` out of release feature sets until a reviewed release build intentionally includes it. PR #890 merged with `Rust (Object Store S3 MinIO)` green, and main push Rust workflow run `29639782907` / job `88068255089` passed the MinIO-backed S3 fixture. A 2026-07-29 local release feature gate now covers default/root/object-store manifest features plus `release.yml` and `release-prebuild.yml` against accidental S3 or `--all-features` enablement; remaining work is the future reviewed release-build decision before intentionally enabling S3.
- [ ] `P2` Harden server-side download ingestion for broad untrusted production exposure: source host allow/deny lists, bounded pre-stream retry, per-host concurrency limits, and structured success/failure logs are implemented; remaining hardening is durable latency/bytes/failure/cleanup counters and an async intent table if product flows need queued, cancelable, or durable failed downloads. Stable contract: [features/object-storage-opendal.md](features/object-storage-opendal.md); notes: [journal/2026-07-22-object-storage-download-ingest.md](journal/2026-07-22-object-storage-download-ingest.md), [journal/2026-07-23-object-storage-download-ingest-implementation.md](journal/2026-07-23-object-storage-download-ingest-implementation.md).

## Observability, CI, And Docs

- [ ] `P2` Continue `features` compaction wave 2: finish a second pass over residual Team/UI micro-journals, extract stable decisions into canonical feature specs, and leave explicit supersession pointers on merged journals so only records with distinct implementation evidence remain. See [features/README.md](features/README.md).

## Maintenance Rules

- Keep only open work here. Remove completed items after evidence lands in a journal, PR, or canonical feature spec.
- Prefer canonical feature specs in [features/](features/) over stale micro-journal references whenever the contract is already stable.
- Collapse duplicated verification bullets into one umbrella matrix when they describe the same rollout surface.
