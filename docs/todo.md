# TODO

Active backlog only. Keep this file small and current.

## Maintenance Rules

- Keep only open work here. Remove completed items after evidence lands in a journal, PR, or canonical feature spec.
- Prefer canonical feature specs in `docs/features/` over stale micro-journal references whenever the contract is already stable.
- Collapse duplicated verification bullets into one umbrella item when they describe the same rollout surface.

## Team Model And Runtime

- [ ] Improve the Team workbench UX around the task-first model: make `Conversation`, `Kanban`, `Agents`, and debug surfaces feel like one coherent workflow, reduce accidental drift back to step/run-first mental models, and tighten the operator path for common Team actions (see `docs/features/agents-teams.md`, `docs/features/frontend-design.md`).
- [ ] Add mobile-first adaptation for Team surfaces so `Conversation`, `Kanban`, agent controls, and runtime/debug entry points remain usable on smaller screens instead of assuming desktop-only layouts (see `docs/features/agents-teams.md`, `docs/features/frontend-design.md`).
- [ ] Strengthen Team realtime delivery beyond the current baseline: shared-thread SSE, Kanban refresh, runtime freshness, and fallback refresh behavior should stay consistent under reconnects, tab restores, and longer-running Team sessions (see `docs/features/agents-teams.md`, `docs/features/frontend-design.md`, `docs/journal/2026-03-19-team-channel-sse-realtime.md`, `docs/journal/2026-03-20-team-channel-sse-state-and-fallback-poll.md`).
- [ ] Continue frontend performance hardening for Team and ACP-heavy pages: reduce avoidable rerenders, keep long lists and live surfaces responsive, and verify the deployed workbench remains fast under realistic Team history sizes (see `docs/features/frontend-design.md`, `docs/features/acp-runtime.md`).
- [ ] Verify the task-first Team model end to end: creating a Team task should not auto-create a run, `team_tasks.assigned_member_id` should stay `NULL` until explicit ownership, and Team docs/skills should keep `run` / `step` framed as execution and debug artifacts instead of the primary collaboration unit (see `docs/features/agents-teams.md`, `docs/journal/2026-03-23-team-task-first-without-backend-orchestrator.md`).
- [ ] Close the gap between the canonical Team task contract and current implementation/UI: leader planning should own task creation and lifecycle, while human conversation remains free-form and Kanban remains the canonical task surface (see `docs/features/agents-teams.md`, `docs/journal/2026-03-19-team-task-ownership-contract.md`).
- [ ] Finish Team runtime freshness tightening so `Start Team` / `Stop Team` update visible runtime state immediately and selected Team runtimes self-recheck while members stay active (see `docs/features/frontend-design.md`, `docs/journal/2026-03-20-team-runtime-freshness-tightening.md`).
- [ ] Verify the deployed Team collaboration surfaces as one joined workflow on `agenthub.hawkingrei.com`: shared-thread SSE and fallback refresh, Kanban task refresh, Agent ACP parity, workspace language cleanup, and agent lifecycle optimistic state should all hold together under normal use (see `docs/features/agents-teams.md`, `docs/features/frontend-design.md`, `docs/journal/2026-03-19-team-channel-sse-realtime.md`, `docs/journal/2026-03-20-team-channel-sse-state-and-fallback-poll.md`, `docs/journal/2026-03-20-team-agent-acp-runtime-optimistic-state.md`, `docs/journal/2026-03-20-team-workspace-language-cleanup.md`).
- [ ] Verify Team ACP permission review routing end to end: `worker -> leader`, `leader -> subordinate worker`, no self-review, and timed-out agent review falling back to a human-visible card in `Conversation` (`all`) with inline actions (see `docs/features/agents-teams.md`, `docs/journal/2026-03-20-team-acp-permission-review-routing.md`, `docs/journal/2026-03-22-team-permission-review-human-fallback-delay.md`, `docs/journal/2026-03-22-team-acp-review-leader-only.md`).
- [ ] Reduce frontend Team prompt mirror drift so prompt previews cannot silently diverge from the canonical backend role prompts (see `docs/features/agents-teams.md`, `docs/journal/2026-03-19-team-task-lifecycle-skill-and-agent-profile-menu.md`).
- [ ] Verify Team agent self-maintenance and deferred follow-up flows: `profile_patch_proposal`, `agent_time_trigger_*`, and operator-controlled `agent_loop` should all behave consistently in Team sessions without blocking normal task progress (see `docs/features/agents-teams.md`, `docs/journal/2026-03-19-team-agent-time-triggers-and-profile-updates.md`, `docs/journal/2026-03-19-team-agent-loop-idle-followup.md`).

## Distributed Execution And Nodes

- [ ] Verify the distributed node phase 0/1 rollout in CI and a real multi-node environment: remote agent start/input/events, mailbox relay plus ack, and node-local data isolation should all hold on both `push` and `pull_request` workflows (see `docs/features/agent-nodes.md`, `docs/features/distributed-node-architecture.md`, `docs/journal/2026-03-18-agent-node-grpc-control-plane.md`, `docs/journal/2026-03-19-distributed-node-architecture.md`).
- [ ] Verify refreshed Agent Node docs against a real multi-node rollout so `configuration-basics`, `agent-nodes`, and `deployment/overview-and-topology` match the current `internal_grpc` contract and remote-node startup path (see `docs/features/agent-nodes.md`, `docs/journal/2026-03-21-agent-node-deployment-doc-refresh.md`).
- [ ] Refactor `ensure_remote_managed_agent` in `src/agent/manager.rs` to remove duplicated legacy-schema `INSERT` / `UPDATE` SQL assembly while preserving schema-compat behavior (see `docs/journal/2026-03-18-agent-node-grpc-control-plane.md`).
- [ ] Strengthen remote-node transport posture beyond the current baseline: validate relay dedupe and timestamp-window policy in staging, design same-port HTTP plus gRPC multiplexing, and define the production identity path (`SPIFFE` / `SPIRE` or equivalent) for long-term mTLS rollout (see `docs/features/distributed-node-architecture.md`).

## ACP And Adapter Hardening

- [ ] Verify `agenthub-codex-acp` default multi-agent enablement after merge and decide whether it should become an explicit AgentHub-owned config knob instead of adapter-owned behavior (see `docs/features/acp-runtime.md`, `docs/journal/2026-03-23-codex-acp-default-multi-agent.md`).
- [ ] Audit `agenthub-codex-acp` provenance and licensing metadata so any MIT-derived material preserves the required notice and repository metadata stays legally accurate before the next release.
- [ ] Verify the codex-acp upstream PR160 sync after the current lockfile upgrade so approvals, `ModelReroute`, and model preset lookup remain compatible across config, options, and model APIs (see `docs/features/acp-runtime.md`, `docs/journal/2026-02-19-codex-acp-upstream-sync-pr160.md`).
- [ ] Establish a focused ACP real-browser regression matrix for long-session behavior: stick-to-bottom, permission history jump/copy, stale-session `send input` recovery, and debug/runtime-metrics surfaces should all remain stable under session switching and long output histories (see `docs/features/acp-runtime.md`, `docs/journal/2026-02-15-acp-conversation-stick-bottom-hardening.md`, `docs/journal/2026-02-16-acp-permission-history-jump-context.md`, `docs/journal/2026-02-16-acp-permission-history-bubble-copy.md`, `docs/journal/2026-02-18-send-input-session-guard-and-acp-type-compat.md`).

## Context, Memory, And Runtime Governance

- [ ] Finalize the Team context and memory continuity design for long-horizon memory: `L0` / `L1` / `L2` ownership, retrieval budget by prompt mode, retention and redaction, promotion rules, and pre-compaction flush behavior need one reviewed v1 contract before further implementation (see `docs/features/agents-teams.md`, `docs/journal/2026-02-22-team-context-memory-architecture.md`, `docs/journal/2026-02-22-team-memory-flush-spec.md`).
- [ ] Verify Team context and memory continuity Track 1 and Track 3 end to end, then close the umbrella milestone only after Track 2 design sign-off is in place (see `docs/journal/2026-02-22-team-context-memory-architecture.md`).
- [ ] Implement workspace-scoped context isolation fully so every agent writes only to its own `<agent_workspace>/.cache/context` tree and cross-agent sharing stays on Team channels and APIs (see `docs/features/agents-teams.md`, `docs/journal/2026-02-22-team-context-memory-architecture.md`).

## CI, Docs, And Maintainability

- [ ] Verify the Bazel CI linker baseline after installing `lld`: `bazel build //...` and `bazel test //...` on both `push` and `pull_request` should stop emitting the Rust gold-linker deprecation warning, and the workflow run IDs should be recorded before closing this item (see `docs/journal/2026-03-24-bazel-ci-lld-linker.md`).
- [ ] Verify Codecov strict-mode uploads and core CI matrices (`Rust`, `Web`, `Web E2E`, distributed node tests, Linux sandbox hardening) on both `push` and `pull_request`, and record run IDs before closing the remaining rollout items (see `docs/journal/2026-02-19-ci-libcap-linux-sandbox.md`, `docs/journal/2026-02-20-ci-codecov-fail-fast-upload.md`, `docs/journal/2026-03-19-distributed-node-architecture.md`).
- [ ] Continue `docs/features` compaction wave 2: move stable conclusions out of residual Team and UI micro-journals into canonical feature specs and replace merged records with explicit supersession pointers where useful (see `docs/features/README.md`).
- [ ] Strengthen documentation governance: every user-visible workflow change should update `userdocs/`, stable engineering contracts should live in `docs/features/`, and `docs/todo.md` should remain an active backlog rather than a historical ledger (see `docs/README.md`, `docs/journal/2026-03-24-documentation-surface-compaction.md`).
- [ ] Verify the refreshed documentation surfaces after deployment: `README.md`, `docs/README.md`, and the published site at `https://doc.agenthub.hawkingrei.com/` should stay aligned with current scripts, configuration shape, Team terminology, and user-facing navigation (see `docs/journal/2026-03-24-documentation-surface-compaction.md`).
