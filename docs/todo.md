# TODO

Active backlog only. Keep this file small and current.

## Release And Packaging

- [ ] `P1` Verify the first live semver npm publish for `@linkerdog/agenthub`: confirm `NPM_TOKEN` has scope publish permission, confirm platform packages publish before the wrapper package, and record the first successful release tag plus npm package URLs in [journal/2026-05-03-npm-release-publish.md](journal/2026-05-03-npm-release-publish.md). Stable contract: [features/npm-binary-distribution.md](features/npm-binary-distribution.md).
- [ ] `P1` Verify the next semver release and preview release publish binary assets even if one target fails, and confirm Linux `x86_64` / `aarch64` release builds bind to vendored OpenSSL instead of the stale cross sysroot OpenSSL. Record workflow run IDs and release URLs in [journal/2026-04-20-release-vendored-openssl-and-partial-assets.md](journal/2026-04-20-release-vendored-openssl-and-partial-assets.md).
- [ ] `P1` Verify the new `Release Prebuild` workflow stays aligned with `release.yml`: it should run after merge on `push` to `main`, exercise the same Linux release feature paths, keep partial-asset publishing semantics, and catch release-only cross regressions before tags are cut. Record workflow run IDs in [journal/2026-04-20-release-vendored-openssl-and-partial-assets.md](journal/2026-04-20-release-vendored-openssl-and-partial-assets.md).

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

## ACP Long-Session Matrix

Stable contracts:

- [features/acp-runtime.md](features/acp-runtime.md)
- [features/runtime-diagnostics.md](features/runtime-diagnostics.md)
- [features/team-conversation-event-bus.md](features/team-conversation-event-bus.md)

Matrix to keep current across real long-running Codex and non-Codex sessions:

- [ ] `P0` Codex full-access/session projection: newly started AgentHub-managed Codex members default to `full-access`, active Team member snapshot/runtime APIs include the open AgentHub `session_id`, and running members do not render as `Starting session` unless their agent status is explicitly `starting`.
- [ ] `P0` Codex ACP permission-deny recovery after PR #604: permission timeout or failed review delivery persists a concrete deny option, submits Codex `ReviewDecision::Denied` instead of `Abort`, avoids prompt abandonment/runtime restart, and surfaces any remaining no-event prompt via `agenthub doctor agent-trace`.
- [ ] `P1` Force New Session recovery: deployed Team member ACP Debug clears persisted session state for that member only, restarts into a fresh ACP session, and runtime-control failures surface member-specific conflict text instead of a generic internal server error.
- [ ] `P1` Long-session browser behavior: stick-to-bottom, permission history jump/copy, stale-session `send input` recovery, debug/runtime-metrics surfaces, and provider session switching remain stable under long output histories.
- [ ] `P1` Provider-native metadata: add provider-native turn/thread ids to `agenthub doctor agent-trace` when an ACP provider adapter can expose them as safe metadata without serializing prompt, message, tool argument, or tool output bodies.
- [ ] `P2` Provider-driven config selectors: verify real Gemini and Kimi ACP sessions render `mode`/`model` select controls when upstream `config_options` are present and can apply `Set Model` / `Set Mode` without manual ID entry.
- [ ] `P2` Polish ACP-native Codex `request_user_input` UX after the inline card rollout: cover richer note-entry flows in browser-level fixtures and decide whether pending questions should bypass prompt-text serialization entirely.

## Team Runtime And Task Model

Stable contracts:

- [features/agents-teams.md](features/agents-teams.md)
- [features/team-execution-vocabulary.md](features/team-execution-vocabulary.md)
- [features/team-workspace-memory-contract.md](features/team-workspace-memory-contract.md)

- [ ] `P0` Verify the task-first Team model end to end: creating a Team task does not auto-create a run, `team_tasks.assigned_member_id` stays `NULL` until explicit ownership, and docs/skills keep `run` / `step` framed as execution and debug artifacts rather than primary collaboration state. Existing notes: [journal/2026-03-23-team-task-first-without-backend-orchestrator.md](journal/2026-03-23-team-task-first-without-backend-orchestrator.md).
- [ ] `P1` Verify Team ACP permission review routing end to end: `worker -> idle peer worker when available, otherwise peer worker/coordinator fallback`, `coordinator -> subordinate worker`, no self-review, and timed-out review falls back to a human-visible card in `# all` with inline actions plus local alert tone.
- [ ] `P1` Verify Team direct-mailbox-first and summary-first communication defaults: single-member mentions route directly, channel `all` fan-out preserves mention metadata, payloads with `detail_ref` preserve concise summaries, and remote recipients route over p2p when their agent targets a remote node.
- [ ] `P1` Finish Team mailbox phase 3: normalize future human/trigger/webhook intake into the canonical inbound-envelope contract, enforce `requires_user_visible_reply` runtime invariants, and surface unresolved reply obligations plus manual takeover state in operator-visible Team UI. Stable contracts: [features/team-mailbox-intake-and-ownership.md](features/team-mailbox-intake-and-ownership.md) and [features/actor-foundation.md](features/actor-foundation.md). Existing notes: [journal/2026-05-20-team-mailbox-phase2-ownership-and-task-links.md](journal/2026-05-20-team-mailbox-phase2-ownership-and-task-links.md).
- [ ] `P1` Continue slimming coordinator/worker prompt tails by moving dynamic runtime state into filesystem-backed memory/index artifacts while preserving role charter, allowed actions, and current-goal gating in prompt text.
- [ ] `P1` Finalize Team context and memory continuity design for long-horizon memory: `L0` / `L1` / `L2` ownership, retrieval budget by prompt mode, retention/redaction, promotion rules, and pre-compaction flush behavior need one reviewed v1 contract before further implementation.
- [ ] `P2` Verify Team agent self-maintenance and deferred follow-up flows: `profile_patch_proposal`, `agent_time_trigger_*`, and operator-controlled `agent_loop` should behave consistently without blocking normal task progress.

## Distributed, Nodes, And Release Matrix

Stable contracts:

- [features/agent-nodes.md](features/agent-nodes.md)
- [features/distributed-node-architecture.md](features/distributed-node-architecture.md)
- [features/distributed-node-registry-and-gossip.md](features/distributed-node-registry-and-gossip.md)
- [features/logical-message-metadata-contract.md](features/logical-message-metadata-contract.md)

Matrix to keep current across CI and at least one real multi-node rollout:

- [ ] `P0` Distributed node phase 0/1 rollout: remote agent start/input/events, mailbox relay plus ack, node-local data isolation, `tests/distributed_p2p_pipeline.rs`, and wire-compatibility tests stay green on both `push` and `pull_request`; record workflow run IDs in the related journal.
- [ ] `P1` Node startup boundaries: `server.role = "node"` boots internal gRPC only, skips main-only startup side effects, and fails fast when `server.node_id` is missing or `internal_grpc.enabled` is false. Existing notes: [journal/2026-04-05-node-mode-startup-boundary.md](journal/2026-04-05-node-mode-startup-boundary.md).
- [ ] `P1` Token-first Agent Node join: root `Agents` surfaces node bootstrap token/details, Admin join exposes token/link without QR onboarding, and user docs match the current `internal_grpc.bootstrap.token` contract. Existing notes: [journal/2026-04-17-node-join-token-flow.md](journal/2026-04-17-node-join-token-flow.md).
- [ ] `P1` Refreshed agent-node deployment docs: `userdocs/docs/deployment/overview-and-topology.md`, `userdocs/docs/core/agent-nodes.md`, and `userdocs/docs/getting-started/configuration-basics.md` match current `internal_grpc` config shape, remote-node registration flow, and remote-target startup behavior.
- [ ] `P1` Team mailbox receive flow: `agenthub actor inbox` and `GET /api/teams/runs/:run_id/messages/inbox` stay read-only with pending messages preserved, while `agenthub actor receive` acknowledges accepted pending items and reduces `pending_count`. Existing notes: [journal/2026-04-02-actor-receive-readonly-inbox.md](journal/2026-04-02-actor-receive-readonly-inbox.md).
- [ ] `P1` Remote-node transport posture: validate relay dedupe and timestamp-window policy in staging, design same-port HTTP plus gRPC multiplexing, and define the production identity path for long-term mTLS rollout.

## Observability, CI, And Docs

- [ ] `P1` Verify deployed Pyroscope bootstrap: full configuration starts one process-wide profiler agent, partial configuration warns and keeps the service running, and shutdown stops the profiler cleanly. Stable contract: [features/pyroscope-profiling.md](features/pyroscope-profiling.md).
- [ ] `P1` Verify the TLS dependency floor refresh: workspace resolution keeps `rustls v0.23.37`, `aws-lc-rs v1.16.2`, and `aws-lc-sys v0.39.0`, and both `cargo check` plus `bazel build //...` stay green. Existing notes: [journal/2026-03-26-rustls-aws-lc-version-floor.md](journal/2026-03-26-rustls-aws-lc-version-floor.md).
- [ ] `P1` Verify the Bazel CI linker baseline after installing `lld`: `bazel build //...` and `bazel test //...` stop emitting the Rust gold-linker deprecation warning, and workflow run IDs are recorded. Existing notes: [journal/2026-03-24-bazel-ci-lld-linker.md](journal/2026-03-24-bazel-ci-lld-linker.md).
- [ ] `P1` Verify Codecov strict-mode uploads and core CI matrices on both `push` and `pull_request`: Rust, Web, Web E2E, distributed node tests, Linux sandbox hardening, and generated-proto coverage exclusions stay aligned.
- [ ] `P2` Continue `features` compaction wave 2: move stable conclusions out of residual Team/UI micro-journals into canonical feature specs and replace merged records with explicit supersession pointers where useful. See [features/README.md](features/README.md).
- [ ] `P2` Verify refreshed documentation surfaces after deployment: `README.md`, `docs/README.md`, and `https://doc.agenthub.hawkingrei.com/` stay aligned with current scripts, configuration shape, Team terminology, and user-facing navigation.

## Maintenance Rules

- Keep only open work here. Remove completed items after evidence lands in a journal, PR, or canonical feature spec.
- Prefer canonical feature specs in [features/](features/) over stale micro-journal references whenever the contract is already stable.
- Collapse duplicated verification bullets into one umbrella matrix when they describe the same rollout surface.
