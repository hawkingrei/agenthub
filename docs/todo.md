# TODO

Active backlog only. Keep this file small and current.

## Release And Packaging

- [ ] `P1` Verify Debian package release artifacts after merge: `Release Prebuild` run `29748545683` proves Linux `agenthub_0.0.0+main_{amd64,arm64}.deb` artifacts are built and uploaded in the prebuild matrix. Remaining evidence is the next release tag including `.deb` files in `SHA256SUMS.txt`. Stable contract: [features/debian-systemd-distribution.md](features/debian-systemd-distribution.md); notes: [journal/2026-06-13-debian-systemd-release-package.md](journal/2026-06-13-debian-systemd-release-package.md).
- [ ] `P1` Verify the first live semver npm publish for `@linkerdog/agenthub`: confirm `NPM_TOKEN` has scope publish permission, confirm platform packages publish before the wrapper package, and record the first successful release tag plus npm package URLs in [journal/2026-05-03-npm-release-publish.md](journal/2026-05-03-npm-release-publish.md). Stable contract: [features/npm-binary-distribution.md](features/npm-binary-distribution.md).
- [ ] `P1` Verify the next semver release and preview release publish binary assets even if one target fails, and confirm Linux `x86_64` / `aarch64` release builds bind to vendored OpenSSL instead of the stale cross sysroot OpenSSL. Record workflow run IDs and release URLs in [journal/2026-04-20-release-vendored-openssl-and-partial-assets.md](journal/2026-04-20-release-vendored-openssl-and-partial-assets.md).

## Team Workspace Browser Matrix

Stable contracts:

- [features/frontend-design.md](features/frontend-design.md)
- [features/workspace-unified-ia.md](features/workspace-unified-ia.md)
- [features/team-channels-threads.md](features/team-channels-threads.md)
- [features/agents-teams.md](features/agents-teams.md)
- [features/teams-collaboration-playbook.md](features/teams-collaboration-playbook.md)

Matrix to keep current on `agenthub.hawkingrei.com` and in PR browser evidence:

- [ ] `P1` Conversation and composer polish: message rows keep the wider Slock-style content lane, human/agent bubbles stay neutral, thread pane reads like the same chat system as the center lane, and channel/thread/ACP composers share one lightweight input language. Existing notes: [journal/2026-04-24-team-conversation-slock-polish.md](journal/2026-04-24-team-conversation-slock-polish.md).
- [ ] `P2` PWA installability: local app-router coverage now proves shell fallbacks, `sw.js`, and `manifest.webmanifest` stay `no-cache`, hashed `/assets/*` stay immutable, and missing asset paths do not fall back to HTML. 2026-07-19 production-domain retries at 07:37 UTC, 09:47 UTC, 11:15 UTC, 12:43 UTC, 13:04 UTC, 13:23 UTC, 13:44 UTC, and 14:12 UTC for `/workspace/teams`, `/sw.js`, `/manifest.webmanifest`, and a missing `/assets/*` probe still returned Cloudflare `502`, which only proves the deployed entrypoint was unavailable. Remaining work is deployed-domain verification that manifest/service-worker remain installable without stale shell caching and that CDN/proxy headers preserve the same contract after the entrypoint is healthy. Stable contract: [features/web-static-assets-and-pwa.md](features/web-static-assets-and-pwa.md); notes: [journal/2026-04-03-pwa-install-and-team-permission-card-collapse.md](journal/2026-04-03-pwa-install-and-team-permission-card-collapse.md), [journal/2026-07-19-pwa-cache-control-router-guard.md](journal/2026-07-19-pwa-cache-control-router-guard.md).

## Team Workspace Architecture

- [ ] `P1` Phase 1/2 unified workspace shell follow-up: PR #891 merged the Team route facade, shared-lens cleanup, channel-scoped member profile routes, compatibility-only legacy query handling, shared workspace shell primitives, and three-zone Team workbench composition without collapsing Team task-first semantics or turning `thread` into a top-level lens. Desktop task-detail preview now reuses the shared split-pane primitive instead of staying modal-only. Remaining work is deeper shell reuse for other persistent Team context docks and deployed browser evidence across Team navigation surfaces. Stable contract: [features/workspace-unified-ia.md](features/workspace-unified-ia.md); notes: [journal/2026-04-18-workspace-shell-route-phase1.md](journal/2026-04-18-workspace-shell-route-phase1.md), [journal/2026-04-18-workspace-shell-phases-1-3-convergence.md](journal/2026-04-18-workspace-shell-phases-1-3-convergence.md), and [journal/2026-07-20-workspace-task-detail-triad.md](journal/2026-07-20-workspace-task-detail-triad.md).
- [ ] `P1` Extend first-run instance setup beyond `agenthub init`: add a reviewed web setup surface, then decide whether provider API base URLs / API keys should become first-class config instead of post-init operator guidance. Stable contract: [features/instance-init-cli.md](features/instance-init-cli.md).
- [ ] `P1` Evaluate explicit Team adoption extensions after the copy-first rollout: design stopped-only `move existing agent to Team`, opt-in workspace-content copy, and opt-in memory/context seeding as separate reviewable modes with runtime, ownership, and history guardrails. Stable contract: [features/team-agent-adoption.md](features/team-agent-adoption.md).
- [ ] `P2` Frontend performance hardening for Team and ACP-heavy pages: reduce avoidable rerenders, keep long lists and live surfaces responsive, and evaluate virtualization/stick-to-bottom behavior for extremely long histories. Stable contracts: [features/frontend-design.md](features/frontend-design.md) and [features/acp-runtime.md](features/acp-runtime.md).

## Team Runtime And Task Model

Stable contracts:

- [features/agents-teams.md](features/agents-teams.md)
- [features/team-execution-vocabulary.md](features/team-execution-vocabulary.md)
- [features/team-workspace-memory-contract.md](features/team-workspace-memory-contract.md)

- [ ] `P1` Verify remote Team direct-mailbox routing on real multi-node teams: after the local API regression and routing fix in [journal/2026-05-26-team-remote-direct-mailbox-routing.md](journal/2026-05-26-team-remote-direct-mailbox-routing.md), confirm direct single-member delivery still preserves mention metadata plus summary/`detail_ref` payloads when the recipient agent is remote and transport falls back to p2p relay in a real multi-node rollout. Existing notes: [journal/2026-03-26-team-direct-mailbox-summary-first.md](journal/2026-03-26-team-direct-mailbox-summary-first.md).
- [ ] `P2` Verify Team agent self-maintenance and deferred follow-up flows: `profile_patch_proposal`, `agent_time_trigger_*`, and operator-controlled `agent_loop` should behave consistently without blocking normal task progress.

## Access Control

Stable contract:

- [features/access-control-and-roles.md](features/access-control-and-roles.md)

- [ ] `P1` Continue user role/capability migration by route cluster: after the first domain matrix, capability helper, bypass guard, remote-node create-agent gate, debug diagnostics `diagnostics:read` route gate, `push:subscribe` route gate, agent-node CRUD/list/get `nodes:manage` gate, admin linker `linkers:manage` gate, agent read/event/permission-list `runtime:inspect` gate, and agent runtime operation `runtime:operate` gate, migrate the remaining normal operator routes from authentication-only auth to capability auth in the order defined by the stable contract. Latest notes: [journal/2026-07-20-diagnostics-capability-gate.md](journal/2026-07-20-diagnostics-capability-gate.md), [journal/2026-07-20-push-subscribe-capability-gate.md](journal/2026-07-20-push-subscribe-capability-gate.md), [journal/2026-07-21-agent-node-capability-gate.md](journal/2026-07-21-agent-node-capability-gate.md), [journal/2026-07-21-linker-capability-gate.md](journal/2026-07-21-linker-capability-gate.md), [journal/2026-07-21-agent-inspect-capability-gate.md](journal/2026-07-21-agent-inspect-capability-gate.md), and [journal/2026-07-21-agent-runtime-capability-gate.md](journal/2026-07-21-agent-runtime-capability-gate.md).

## Message Storage

- [ ] `P1` Implement the RocksDB `cf_index` delivery projection and its authority-derived repair path, per [features/message-storage-tiering.md](features/message-storage-tiering.md). The opt-in Phase 1 `cf_body` dual-write/backfill is complete; keep normal reads on SQLite until ordered index reads, integrity checks, and backup validation are in place. Do not drop SQLite bodies before the Phase 2 rollout decision.

## Object Storage

- [ ] `P1` Decide whether browser-facing multipart or presigned upload tokens should become the canonical large-object path after the Team, task, and agent JSON/base64 owner-scope routes. Stable contract: [features/object-storage-opendal.md](features/object-storage-opendal.md); notes: [journal/2026-07-16-object-storage-opendal.md](journal/2026-07-16-object-storage-opendal.md), [journal/2026-07-18-object-upload-owner-scopes.md](journal/2026-07-18-object-upload-owner-scopes.md).
- [ ] `P1` Keep `agenthub-object-store/s3` out of release feature sets until a reviewed release build intentionally includes it. PR #890 merged with `Rust (Object Store S3 MinIO)` green, and main push Rust workflow run `29639782907` / job `88068255089` passed the MinIO-backed S3 fixture.

## Observability, CI, And Docs

- [ ] `P2` Continue `features` compaction wave 2: finish a second pass over residual Team/UI micro-journals, extract stable decisions into canonical feature specs, and leave explicit supersession pointers on merged journals so only records with distinct implementation evidence remain. See [features/README.md](features/README.md).

## Maintenance Rules

- Keep only open work here. Remove completed items after evidence lands in a journal, PR, or canonical feature spec.
- Prefer canonical feature specs in [features/](features/) over stale micro-journal references whenever the contract is already stable.
- Collapse duplicated verification bullets into one umbrella matrix when they describe the same rollout surface.
