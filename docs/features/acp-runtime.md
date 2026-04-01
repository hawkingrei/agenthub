# ACP Runtime Specification

## Problem

ACP behavior spans backend runtime integration, streaming transport, frontend conversation/debug surfaces,
and permission workflows. Historical updates were recorded in date-based notes, but a stable ACP domain
spec is required to keep contracts consistent across providers and UI/runtime layers.

## Scope

- ACP runtime module boundaries and bootstrap integration.
- ACP event ingestion/rendering model in web conversation/debug surfaces.
- ACP transport/reliability behavior (streaming + fallback).
- ACP permission workflow and scoping guarantees.
- ACP provider compatibility baseline for Codex/Gemini/Kimi adapters.

## Non-Goals

- Replacing provider-specific adapter implementations.
- Re-documenting every ACP UI polish change as timeline history.
- Defining Team orchestration semantics beyond ACP interaction boundaries.

## Architecture

### 1) Runtime Module Boundary

ACP runtime is managed as a package-style module with clear boundaries:

- `src/acp/mod.rs`
- `src/acp/event_sink.rs`
- `src/acp/runtime.rs`

Service bootstrap remains thin and keeps ACP contracts stable to callers.

### 2) Provider / Placement / Proxy Layering

ACP runtime concerns should stay split across three orthogonal layers:

- Provider adapter layer:
  - Detects whether a command speaks ACP.
  - Owns provider-specific defaults and prompt-delivery semantics.
  - Examples today: Codex, Gemini, Kimi.
- Runtime placement layer:
  - Decides where the ACP runtime executes and how AgentHub connects to it.
  - The current baseline is explicit local-process execution.
  - Startup planning should stay explicit so "reuse running session", "start local", and "start remote" remain a named boundary instead of an inlined branch chain.
  - Local subprocess launch should stay behind an executor seam so AgentHub can add remote placements without re-expanding manager logic.
  - Future remote-node/P2P support must extend this axis instead of adding provider-specific transport stacks.
- Proxy policy layer:
  - Applies egress policy (`HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, `NO_PROXY`) to a chosen runtime placement.
  - Must stay provider-agnostic so local and remote runtimes reuse the same policy model.

### 3) End-to-End Event Path

1. ACP provider emits runtime events/tool-call outputs.
2. Backend event sink normalizes and persists stream artifacts.
3. Frontend ACP conversation/debug surfaces consume ordered events.
4. UI applies rendering policies (group/fold/humanized payloads/permission linking).

### 4) Conversation/Debug Surfaces

- Conversation is the primary event timeline with ordered render.
- Debug provides advanced state/introspection:
  - permission pending/history
  - runtime metrics
  - raw events and jump/copy operations

### 5) Transport Model

- Streaming path uses SSE for near-real-time updates.
- Polling/history paths provide fallback/replay continuity.
- Multi-agent subscriptions and bounded fan-in behavior should avoid stream overload.
- When persisted agent state says `running` but the runtime handle is missing after abrupt shutdown/restart,
  backend transport paths should reconcile stale state to `exited` instead of letting SSE retry indefinitely
  against a non-existent runtime.

### 6) Permission Workflow

ACP permission requests are first-class runtime records:

- request list/history
- scoped rendering to active agent/session
- copy/jump interactions to trace request <-> conversation context

## Contracts

### 1) Ordering Contract

- ACP conversation rendering must preserve deterministic event ordering.
- Tool/thinking/output blocks must remain stable under mixed event bursts.
- Replay history and live stream should converge to one ordered view.

### 2) Stick-Bottom And Virtualization Contract

- Auto-stick should follow new tail updates when user is near bottom.
- Manual scroll-up should disable forced jump until explicit return.
- Long sessions should use bounded rendering/virtualization to control DOM/CPU cost.
- Text-dominant conversation rows may use a text-aware height estimator to reduce spacer drift, but
  non-text/tool rows may still fall back to coarse estimates and must never block rendering on DOM
  measurement.

### 3) Permission Scope Contract

- Permission pending/history must be scoped to active agent context.
- Agent switch must clear stale permission view state immediately.
- Late async responses from previous agent context must be ignored.

### 4) Session Safety Contract

- Input/send paths must validate session alignment to avoid stale session writes.
- Session mismatch and unavailable gateway states should surface deterministic, non-leaky errors.
- `agent_sessions.id` is AgentHub's per-launch runtime/audit identifier and is expected to change on
  each real restart.
- `agent_persistent_sessions.session_id` stores provider continuity identity separately so ACP/Codex
  memory can survive across multiple AgentHub launches until an explicit reset path clears it.
- Stable project workdirs must not be keyed by the per-launch runtime session id; workspace identity
  and provider continuity are separate concerns.
- Resumed Codex sessions must tolerate dirty rollout history:
  - if a stored `CustomToolCall`, `FunctionCall`, or shell-call item is missing its matching output
    item, the adapter should repair the history with a synthetic aborted output before resume
  - orphan output items without a matching call should be dropped during the same repair pass
  - history repair should happen before compaction/normalization can panic on missing outputs

### 5) Transport Recovery Contract

- SSE subscription failure caused by missing runtime ownership should trigger stale-state reconciliation when safe.
- Best-effort process shutdown should converge visible `running` agent/session rows before exit.
- SSE `404 agent not running` responses should represent converged runtime truth, not stale DB state that keeps clients retrying forever.

### 6) Provider Compatibility Contract

- ACP protocol mapping must stay aligned with upstream provider schemas.
- Codex ACP sync changes should preserve session listing, tool-call payload decode, and event handling contracts.
- Gemini/Kimi ACP presets should preserve session clear and provider-specific defaults without regressing core ACP flow.
- Gemini CLI bootstrap should track the current upstream ACP contract (`gemini --acp`) while continuing to tolerate the legacy `--experimental-acp` flag in provider detection for backward compatibility.
- When an ACP provider returns `auth_required`, AgentHub should surface an explicit setup error instead of silently retrying interactive auth flows on behalf of a remote user.

### 7) Placement And Proxy Contract

- Provider identity and runtime placement must stay independent axes.
- Introducing remote-node/P2P execution must not require duplicating Codex/Gemini/Kimi adapter logic.
- Proxy handling must remain a provider-agnostic launch policy that can be applied to both local and future remote runtimes.

## Validation Matrix

- `pnpm -C web run lint`
- `pnpm -C web run build`
- `pnpm -C web exec vitest run src/acp_panel.test.tsx src/acp_debug.test.tsx src/acp_conversation_render.test.tsx src/acp_conversation.interaction.test.tsx src/hooks/use_acp_conversation.test.ts`
- `cargo check -p agenthub-codex-acp`
- `cargo test -p agenthub-codex-acp`

## Operational Notes

- Keep ACP contracts provider-agnostic at system boundary; isolate provider drift in adapter modules.
- Prefer additive compatibility changes when protocol evolves.
- AgentHub owns Codex subagent enablement through `codex_acp.multi_agent_enabled` (default
  `true`). When launching `agenthub-codex-acp`, AgentHub should pass an explicit
  `AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED=1|0` child-process env override so ACP sessions expose
  Codex `Feature::Collab` deterministically without depending on per-user
  `~/.codex/config.toml` toggles.
- `agenthub-acp` should materialize AgentHub-managed Codex skills under
  `~/.agents/skills/agenthub-runtime/.../SKILL.md` during ACP session bootstrap, then inject those
  file-backed skills through ACP `<skill>` wrappers so `agenthub-codex-acp` can translate them into
  native Codex `UserInput::Skill` items.
- Global managed-skill paths should stay on the home-rooted forms only: canonical absolute paths,
  with `~/...` accepted as a compatibility spelling before translation. Repo-local
  `<workdir>/.agents/skills/**/SKILL.md` remains an independent discovery path and should not be
  rewritten into the managed global namespace.
- `agenthub-codex-acp` must keep ACP request/response I/O alive on a dedicated runtime thread so
  ACP-backed filesystem reads can safely round-trip while tool handlers synchronously verify or
  patch existing files.
- Dynamic actor runtime fields such as `team_id`, `current_run_id`, and continuity summaries should
  stay in a separate text prefix block injected before each prompt instead of being rewritten into
  the managed `SKILL.md` files.
- Keep debug capabilities available without exposing internal-only controls in primary user path.
- Treat in-memory runtime ownership as authoritative for live SSE; use persisted status as a recoverable cache that may require reconciliation after abrupt exits.
- Keep provider metadata, runtime placement, and proxy policy explicit in code so future P2P work extends stable seams instead of forking provider-specific paths.
- Team/operator recovery may explicitly clear a persisted ACP session and force a new session for a
  selected member when provider history is irrecoverably dirty; this should remain a targeted
  recovery path, not the normal resume flow.

## Open Risks

- Upstream protocol drift may still require frequent adapter sync and lockfile refresh.
- Long-session rendering can regress if virtualization/stick-bottom heuristics are bypassed.
- Permission UX still needs periodic real-browser verification under rapid agent switching.

## Source Journals

- `docs/journal/2026-02-20-acp-package-and-bootstrap-module-migration.md`
- `docs/journal/2026-02-09-codex-acp-protocol-sync.md`
- `docs/journal/2026-02-15-acp-conversation-stick-bottom-hardening.md`
- `docs/journal/2026-02-16-permission-history-agent-scope.md`
- `docs/journal/2026-02-20-web-tailwind-ui-phase8-acp-panel-debug-shell.md`
- `docs/journal/2026-02-20-web-tailwind-ui-phase9-acp-conversation-shell.md`
- `docs/journal/2026-03-08-sse-stale-running-agent-reconciliation.md`
- `docs/journal/2026-03-22-acp-provider-runtime-abstraction.md`
- `docs/journal/2026-03-24-codex-acp-native-skill-injection.md`
- `docs/journal/2026-03-30-codex-acp-apply-patch-deadlock.md`
- `docs/journal/2026-03-31-pretext-acp-conversation-virtualization.md`
