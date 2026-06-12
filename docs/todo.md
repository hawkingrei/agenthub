# TODO

Active backlog only. Keep this file small and current.

## Release And Packaging

- [ ] `P1` Verify the first live semver npm publish for `@linkerdog/agenthub`: confirm `NPM_TOKEN` has scope publish permission, confirm platform packages publish before the wrapper package, and record the first successful release tag plus npm package URLs in [journal/2026-05-03-npm-release-publish.md](journal/2026-05-03-npm-release-publish.md). Stable contract: [features/npm-binary-distribution.md](features/npm-binary-distribution.md).
- [ ] `P1` Verify the next semver release and preview release publish binary assets even if one target fails, and confirm Linux `x86_64` / `aarch64` release builds bind to vendored OpenSSL instead of the stale cross sysroot OpenSSL. Record workflow run IDs and release URLs in [journal/2026-04-20-release-vendored-openssl-and-partial-assets.md](journal/2026-04-20-release-vendored-openssl-and-partial-assets.md).
- [ ] `P1` Verify the trimmed `Release Prebuild` workflow after the `agenthub-codex-acp` release artifact removal lands: it should run after merge on `push` to `main`, publish only `agenthub` and `agenthub-acp` prebuild archives, exercise the same Linux release feature paths, and catch release-only cross regressions before tags are cut. Record workflow run IDs in [journal/2026-04-20-release-vendored-openssl-and-partial-assets.md](journal/2026-04-20-release-vendored-openssl-and-partial-assets.md).

## Team Workspace Browser Matrix

Stable contracts:

- [features/frontend-design.md](features/frontend-design.md)
- [features/workspace-unified-ia.md](features/workspace-unified-ia.md)
- [features/team-channels-threads.md](features/team-channels-threads.md)
- [features/agents-teams.md](features/agents-teams.md)
- [features/teams-collaboration-playbook.md](features/teams-collaboration-playbook.md)

Matrix to keep current on `agenthub.hawkingrei.com` and in PR browser evidence:

- [ ] `P1` Conversation and composer polish: message rows keep the wider Slock-style content lane, human/agent bubbles stay neutral, thread pane reads like the same chat system as the center lane, and channel/thread/ACP composers share one lightweight input language. Existing notes: [journal/2026-04-24-team-conversation-slock-polish.md](journal/2026-04-24-team-conversation-slock-polish.md).
- [ ] `P1` Team shell refinement and route chunk split: selector remains a slim chooser, Team/ACP shells stay thin and wide, read-state indicators remain unobtrusive, and deployed `index.html` does not preload route chunks that should be lazy. Existing notes: [journal/2026-03-21-team-ui-shell-and-bundle-refinement.md](journal/2026-03-21-team-ui-shell-and-bundle-refinement.md).
- [ ] `P1` Agents workbench lazy-split LCP: deployed `agents` route should ship a small primary shell chunk while OutputBody/InputDock/ACP workbench code loads lazily; evaluate `AcpDebug` as a follow-up split if it still dominates. Existing notes: [journal/2026-04-06-agents-lcp-workbench-split.md](journal/2026-04-06-agents-lcp-workbench-split.md).
- [ ] `P2` PWA installability: deployed manifest/service-worker remain installable without stale shell caching; HTML shell routes, `sw.js`, and `manifest.webmanifest` stay `no-cache`, while `/assets/*` remains immutable. Existing notes: [journal/2026-04-03-pwa-install-and-team-permission-card-collapse.md](journal/2026-04-03-pwa-install-and-team-permission-card-collapse.md).

## Team Workspace Architecture

- [ ] `P1` Phase 1/2 unified workspace shell follow-up: finish route parity, deeper shell reuse, and shared-lens cleanup without collapsing Team task-first semantics or turning `thread` into a top-level lens. Stable contract: [features/workspace-unified-ia.md](features/workspace-unified-ia.md); notes: [journal/2026-04-18-workspace-shell-route-phase1.md](journal/2026-04-18-workspace-shell-route-phase1.md) and [journal/2026-04-18-workspace-shell-phases-1-3-convergence.md](journal/2026-04-18-workspace-shell-phases-1-3-convergence.md).
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

## Message Storage

- [ ] `P1` Implement message-body compression via RocksDB, per [features/message-storage-tiering.md](features/message-storage-tiering.md). Move message bodies out of SQLite authority rows into a RocksDB `cf_body` column family compressed by SST block compression (plain zstd + bottommost zstd; trained dictionary deferred), keep a body-free RocksDB delivery index, and stage migration as dual-body write + backfill (Phase 1) then drop the SQLite body column (Phase 2). Goal is shrinking at-rest chat/message storage; SQLite stays metadata authority, LanceDB stays the (eventually-consistent) search layer. First PR boundary: opt-in RocksDB backend behind a feature flag with `cf_body` round-trip + compression-ratio tests on a real chat corpus, before any SQLite body drop.

## Observability, CI, And Docs

- [ ] `P2` Continue `features` compaction wave 2: finish a second pass over residual Team/UI micro-journals, extract stable decisions into canonical feature specs, and leave explicit supersession pointers on merged journals so only records with distinct implementation evidence remain. See [features/README.md](features/README.md).

## Maintenance Rules

- Keep only open work here. Remove completed items after evidence lands in a journal, PR, or canonical feature spec.
- Prefer canonical feature specs in [features/](features/) over stale micro-journal references whenever the contract is already stable.
- Collapse duplicated verification bullets into one umbrella matrix when they describe the same rollout surface.
