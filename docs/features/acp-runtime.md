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
- ACP provider compatibility baseline for Codex/Gemini/Kimi/Claude adapters, including
  AgentHub-distributed adapter binaries where available.
- Codex-specific diagnostic side-channel boundaries for live turn/tool-call integrity.
- Standalone ACP workbench behavior, including runtime context and Codex subagent visibility.
- Local ACP multimodal image input and provider-generated image output.

## Non-Goals

- Replacing provider-specific adapter implementations.
- Replacing AgentHub's provider-neutral ACP control/event boundary with Codex-only app-server
  protocol calls.
- Re-documenting every ACP UI polish change as timeline history.
- Defining Team orchestration semantics beyond ACP interaction boundaries.
- Extending image attachments to remote agents, stdin-only processes, or Team composers.

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
  - Examples today: Codex, Gemini, Kimi, Claude.
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

Codex and Claude are special cases inside the provider adapter layer:

- `agenthubd acp codex` is AgentHub's canonical distributed Codex ACP adapter path. It uses the
  upstream Codex app-server/thread protocol internally. AgentHub's main process and web surfaces
  should continue to consume ACP requests, ACP notifications, and AgentHub-normalized ACP event JSON
  as the stable boundary. Codex-native state may be exposed only through explicit diagnostics
  metadata, not by making the primary runtime path Codex-specific.
- `agenthubd acp claude` is AgentHub's distributed Claude ACP adapter path. It wraps the Rust
  `claude-code-acp-rs` library and always starts ACP server mode for AgentHub-managed sessions,
  while still leaving Claude credentials and model settings in the adapter-supported Anthropic
  environment or Claude settings files. The daemon is provider-selected by the internal `acp`
  subcommand so additional built-in ACP adapters can share the same native file later.

### 4) Conversation/Debug Surfaces

- Conversation is the primary event timeline with ordered render.
- The standalone ACP workbench exposes the active run state, model, thinking level, permission mode,
  and Codex subagent activity without requiring the debug tab.
- Codex collaboration and subagent events remain ACP tool activity. They must not be projected as
  AgentHub Team members or mutate Team ownership and execution records.
- Codex runtime profiles accept `xhigh`, `max`, and `ultra` in addition to the common reasoning
  levels. The adapter must pass these values through exactly; provider/model validation remains
  authoritative. Claude retains the common `low`, `medium`, `high`, and `max` profile levels.
- Asynchronous Codex agent messages remain in the ordered conversation and carry an explicit
  background-delivery label instead of being mistaken for synchronous turn output.
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
- timeout/failure settlement through the permission-response channel, not through prompt/session
  cancellation

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
- Permission review timeout is a deny decision, not an ACP prompt cancellation. AgentHub should
  choose a concrete reject option when one exists, persist that selected option, and return it as a
  `RequestPermissionResponse`.
- `RequestPermissionOutcome::Cancelled` is reserved for true prompt/session cancellation or for the
  degenerate case where the provider supplied no reject option. It must not be the normal timeout or
  review-dispatch-failure result.
- A denied permission may still emit a terminal failed tool-call update for UI clarity, but that
  synthetic UI event does not replace the provider's prompt completion. AgentHub must keep draining
  the provider prompt future until it returns or fails.
- Permission denial must not send `CancelNotification`, locally abandon the active prompt, kill the
  ACP process, or restart the agent. Explicit user interrupt/cancel remains the only path that maps
  to provider prompt/session cancellation.

### 4) Session Safety Contract

- Input/send paths must validate session alignment to avoid stale session writes.
- Session mismatch and unavailable gateway states should surface deterministic, non-leaky errors.
- If AgentHub has already persisted a user input event but cannot submit the prompt command to the
  ACP provider, it should append a redacted visible ACP status event and keep detailed failure
  metadata in diagnostics rather than serializing prompt bodies or provider internals into the
  conversation.
- `agent_sessions.id` is AgentHub's per-launch runtime/audit identifier and is expected to change on
  each real restart.
- Team member snapshot/runtime APIs must expose the current open AgentHub `session_id` for active
  members. A running member with an open `agent_sessions` row must not be rendered as
  `Starting session` just because one refresh path temporarily missed the session projection.
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
- The Codex ACP baseline is the official `openai/codex` release source. AgentHub must pin Cargo and
  Bazel to one reviewed upstream commit and must not require a downstream Codex fork for protocol or
  dependency compatibility.
- Codex ACP sync changes should preserve session listing, tool-call payload decode, and event handling contracts.
- Codex ACP behavior should use upstream Codex TUI/core behavior as the compatibility reference
  when ACP semantics are ambiguous. In particular, a permission denial is a model-visible failed
  tool result, not a turn cancellation, and follow-up user input may be steered into the same
  active turn after permission gates clear.
- Codex ACP ordinary prompt delivery should allow follow-up prompts to reach the Codex app-server
  while a steerable turn is still active, so Codex can append the new input to that turn instead of
  forcing AgentHub to wait for a terminal turn event. AgentHub still queues prompts while a session
  mutation is pending or while a permission request is pending. AgentHub must not use its pending
  tool-call diagnostic as an outer prompt-dispatch gate for Codex; Codex owns tool/turn ordering.
- ACP cancel is the explicit interrupt path and must not wait behind active prompts.
- Team-managed Codex ACP sessions default to `full-access` mode so Team workers do not depend on
  permission-review round trips for ordinary repository operations. Permission deny/timeout
  semantics still matter for explicit provider permission requests and must not be implemented as
  turn abort/cancel.
- Codex ACP startup permission mode is an agent configuration, not a live-session control. The
  canonical persisted field is `agents.codex_acp_default_mode`; `full-access` is the default and
  `yolo` is accepted as an alias. Updating this setting must not mutate the active ACP provider
  session; the selected mode applies only after the agent is restarted.
- Team-managed Codex members inherit `full-access` unless their persisted
  `codex_acp_default_mode` overrides it. The web UI may label the default as
  `Yolo / full access`, but stored/runtime values must stay canonicalized to `full-access`.
- Codex ACP live-turn diagnostics should track enough native app-server state to explain a stuck or
  panicking turn without changing the provider-neutral ACP surface:
  - active Codex thread id, turn id, and AgentHub submission id
  - queued submission count and pending app-server request ids
  - pending tool calls keyed by Codex `call_id` / ACP `tool_call_id`
  - whether each custom tool call has observed a matching output item before turn completion,
    compaction, resume, or shutdown
  - the last Codex `EventMsg` class and timestamp seen by the adapter
- Provider-native metadata exposed through `agenthub doctor agent-trace` must use an explicit
  allowlist. Safe fields are limited to identifiers and counters needed to correlate provider
  state, such as ACP session id, provider session id, Codex thread/turn/item/request/submission ids,
  tool-call/permission ids, event classes, timestamps, and queue counts. Diagnostics must not
  serialize prompt text, message bodies, tool arguments, tool outputs, command error messages, or
  arbitrary provider JSON fields.
- Codex `ReviewDecision::Abort` is a whole-turn interrupt in upstream Codex core. AgentHub's
  provider adapter must not expose it as the default "No" permission-review option and must not map
  permission timeout or failed review delivery to it. Ordinary denial should map to
  `ReviewDecision::Denied` so Codex can continue the turn and decide the next model-visible step.
- Codex `RequestPermissions` denial should return an empty permission profile scoped to the turn,
  not an abort/cancel outcome.
- Gemini/Kimi/Claude ACP presets should preserve session clear and provider-specific defaults without regressing core ACP flow.
- Gemini CLI bootstrap should track the current upstream ACP contract (`gemini --acp`) while continuing to tolerate the legacy `--experimental-acp` flag in provider detection for backward compatibility.
- Codex ACP support has a canonical AgentHub-distributed command: `agenthubd acp codex`.
  Compatibility detection still recognizes `agenthub-codex-acp` and `codex-acp` as Codex ACP
  runtimes for existing deployments and custom overrides, but current AgentHub release assets should
  publish `agenthubd` instead of either legacy adapter binary.
- Claude ACP support has a canonical AgentHub-distributed command: `agenthubd acp claude`.
  Compatibility detection still recognizes `claude-agent-acp` as an ACP runtime directly, and
  recognizes `claude-code-acp-rs` only when launched with `--acp` so headless or diagnostic Claude
  Code invocations are not misclassified as interactive ACP sessions.
- When an ACP provider returns `auth_required`, AgentHub should surface an explicit setup error instead of silently retrying interactive auth flows on behalf of a remote user.

### 7) Multimodal Contract

- The standalone browser composer may submit text, images, or both. Image-only prompts must not add
  an empty text content block.
- The browser sends validated image attachments through the existing agent input route. AgentHub
  maps them to ACP `ImageContent`, and the Codex adapter maps that provider-neutral content to Codex
  image user input.
- Input is limited to four images, 5 MiB per decoded image, and 10 MiB total decoded image bytes.
  The input route has a 16 MiB request-body limit to cover JSON and base64 overhead.
- Accepted formats are PNG, JPEG, WebP, and GIF. The backend must validate standard base64, declared
  MIME type, and the corresponding file signature before persisting or forwarding an attachment.
- Multimodal input is currently local-ACP-only. Remote agents and stdin-only processes must reject
  image attachments deterministically instead of silently dropping them.
- The normalized user event keeps one bounded attachment copy so SSE replay can reconstruct the
  conversation. Prompt submission failures must preserve the visible user event and attachments.
- Provider-generated images are represented as ACP image content with an optional saved-resource
  link. Raw/debug tool output must not duplicate the base64 payload.
- The web renderer may display only PNG, JPEG, WebP, and GIF from validated `data:` or HTTP(S)
  sources. SVG and local-file URI rendering remain disallowed.

### 8) Standalone Workbench Contract

- Image selection, clipboard paste, drag-and-drop, preview, removal, and image-only submission are
  available in the standalone ACP composer without changing Team composer semantics.
- Composer validation must mirror backend limits and preserve attachments across failed sends or
  explicit retries. Successful sends clear text and attachments together.
- The primary header exposes run state, model, live reasoning effort, effective permission mode, and
  active versus total Codex subagent counts in a compact, localization-ready presentation. Live ACP
  config values take precedence over persisted startup defaults.
- Repeated Codex subagent activity with the same thread identifier updates one deterministic tool
  card. Live subagent cards must not be closed by generic stale-tool settlement.

### 9) Placement And Proxy Contract

- Provider identity and runtime placement must stay independent axes.
- Introducing remote-node/P2P execution must not require duplicating Codex/Gemini/Kimi/Claude adapter logic.
- Proxy handling must remain a provider-agnostic launch policy that can be applied to both local and future remote runtimes.

## Validation Matrix

- `pnpm -C web run lint`
- `pnpm -C web run build`
- `pnpm -C web exec vitest run src/acp_panel.test.tsx src/acp_debug.test.tsx src/acp_conversation_render.test.tsx src/acp_conversation.interaction.test.tsx src/hooks/use_acp_conversation.test.ts`
- `cargo check -p agenthub-acp-adapter`
- `cargo test -p agenthub-acp-adapter`
- `cargo check -p agenthub-codex-acp-runtime`
- `cargo test -p agenthub-codex-acp-runtime`
- `cargo test -p agenthub-acp prompt_`
- `cargo test -p agenthub input_image_validation`
- `cargo test -p agenthub-config normalize_optional_thinking_level`
- `cargo test -p agenthub codex_reasoning_effort_maps_thinking_levels`
- `cargo test -p agenthub create_agent_route_`
- `pnpm -C web exec vitest run src/acp.test.ts src/components/input_dock.test.tsx src/components/acp_media_gallery.test.tsx src/agents_workbench.test.tsx src/components/use_agents_workbench_panel.test.tsx src/api.test.ts src/create_agent_modal.test.tsx src/pages/team/team_management_modals.test.tsx`
- Focused `agenthub-codex-acp` tests for live-turn tool-call completeness:
  - a `CustomToolCall` without matching `CustomToolCallOutput` is recorded as diagnostic state
    before turn completion/compaction can panic
  - orphan custom-tool outputs are reported without corrupting ACP event replay
  - ordinary Codex prompts are dispatched for app-server turn steering once permission gates clear,
    while ACP cancel remains dispatchable as an interrupt command
- Focused `agenthub-codex-acp` permission-decision tests:
  - Codex exec approval options derived from upstream `Abort` are exposed to AgentHub as a deny
    option and submit `ReviewDecision::Denied`
  - patch approval rejection submits `ReviewDecision::Denied`, not `ReviewDecision::Abort`
  - `RequestPermissions` rejection submits an empty turn-scoped permission response
  - denied approvals produce model-visible failed tool results, matching Codex TUI/core recovery
    semantics rather than cancelling the active turn
- Focused `agenthub-acp` permission-runtime tests:
  - timeout selects a concrete reject option when available and persists `selected_option_id`
  - denied permission tool-call settlement clears pending tool-call diagnostics without cancelling
    the prompt
  - stale-prompt diagnostics stay suppressed while a permission is pending and activate only after
    pending permissions have resolved
- Focused `agenthub` API/web tests:
  - create-agent and update-config routes canonicalize `yolo` to `full-access`
  - updating `codex_acp_default_mode` changes persisted agent configuration without restarting or
    controlling the active provider session
  - web create/edit controls send the startup mode and label it as restart-required configuration
- Focused `agenthub-diagnostics` / `agenthub` tests:
  - `agent-trace` event summaries extract only allowlisted provider-native ids from persisted ACP
    JSON and mark body-like fields as redacted
  - live ACP provider diagnostics preserve safe ids/counters while omitting command error messages
    and other body-like text

## Operational Notes

- Keep ACP contracts provider-agnostic at system boundary; isolate provider drift in adapter modules.
- Prefer additive compatibility changes when protocol evolves.
- Keep the Codex Cargo and Bazel pins on the same official `openai/codex` commit. A dependency graph
  regression should be tracked and remediated upstream or through ordinary dependency constraints,
  not by silently switching AgentHub back to a downstream Codex fork.
- AgentHub owns Codex subagent enablement through `codex_acp.multi_agent_enabled` (default
  `true`). When launching Codex ACP through `agenthubd acp codex`, `agenthub-codex-acp`, or another
  recognized Codex ACP command, AgentHub should pass an explicit
  `AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED=1|0` child-process env override so ACP sessions expose
  Codex `Feature::Collab` deterministically without depending on per-user `~/.codex/config.toml`
  toggles.
- The daemon's Codex ACP worker should materialize AgentHub-managed Codex skills under
  `~/.agents/skills/agenthub-runtime/.../SKILL.md` during ACP session bootstrap, then inject those
  file-backed skills through ACP `<skill>` wrappers so the Codex adapter can translate them into
  native Codex `UserInput::Skill` items.
- Global managed-skill paths should stay on the home-rooted forms only: canonical absolute paths,
  with `~/...` accepted as a compatibility spelling before translation. Repo-local
  `<workdir>/.agents/skills/**/SKILL.md` remains an independent discovery path and should not be
  rewritten into the managed global namespace.
- The Codex ACP adapter must keep ACP request/response I/O alive on a dedicated runtime thread so
  ACP-backed filesystem reads can safely round-trip while tool handlers synchronously verify or
  patch existing files.
- Dynamic actor runtime fields such as `team_id`, `current_run_id`, and continuity summaries should
  stay in a separate text prefix block injected before each prompt instead of being rewritten into
  the managed `SKILL.md` files.
- That runtime continuity block should stay pointer-first: keep a compact continuity summary in
  prompt text, and route deeper history through persisted artifacts or replay state instead of
  embedding raw continuity windows inline.
- Keep debug capabilities available without exposing internal-only controls in primary user path.
- Treat in-memory runtime ownership as authoritative for live SSE; use persisted status as a recoverable cache that may require reconciliation after abrupt exits.
- Keep provider metadata, runtime placement, and proxy policy explicit in code so future P2P work extends stable seams instead of forking provider-specific paths.
- Team/operator recovery may explicitly clear a persisted ACP session and force a new session for a
  selected member when provider history is irrecoverably dirty; this should remain a targeted
  recovery path, not the normal resume flow.
- Treat Codex app-server/protocol state as an adapter-local observability source. It can explain
  Codex-specific failures such as missing custom-tool outputs, but it should not become the primary
  AgentHub runtime contract while Gemini/Kimi and future providers still rely on ACP.
- Treat provider-native diagnostics as a correlation aid, not a data dump. Adding a new field to
  `agent-trace` requires updating the allowlist and confirming the field cannot carry prompt,
  message, tool argument, or tool output bodies.
- When a Codex ACP agent appears stuck after permission timeout, first verify the option id and
  submitted Codex decision. If the persisted option is `abort` or the adapter submits
  `ReviewDecision::Abort`, the adapter is using interrupt semantics and should be fixed before
  adding recovery timers.
- A pending mailbox message after a stale active prompt is usually a downstream symptom: the mailbox
  hint cannot be delivered because prompt serialization is still occupied. Diagnose the active
  provider prompt and permission gate before treating mailbox delivery as the root cause.
- Synthetic AgentHub events should be labeled and reasoned as synthetic UI/runtime settlement. They
  must not be counted as proof that the provider emitted a new event or that a prompt completed.

## Open Risks

- Upstream protocol drift may still require frequent adapter sync and lockfile refresh.
- Live Codex tool-call completeness can still drift from ACP-visible tool-call rendering if the
  adapter only validates persisted rollout history. The live path needs its own accounting before
  compaction or resume paths run.
- Long-session rendering can regress if virtualization/stick-bottom heuristics are bypassed.
- Permission UX still needs periodic real-browser verification under rapid agent switching.
- Inline base64 attachment persistence increases SQLite and SSE replay cost near the current input
  limits. A later storage migration should replace inline event payloads with owner-scoped object
  references and replay-safe thumbnails without changing the ACP content contract.
- Official Codex releases can reintroduce older or parallel transitive dependency versions. Each
  Codex upgrade needs a lockfile audit even when the upstream source itself is current.
- A provider may still fail to finish after receiving a valid deny response. AgentHub should expose
  that as a stale provider prompt with safe diagnostics; automatic cancel, kill, or prompt
  abandonment remains out of scope unless a future provider-specific contract explicitly requires it.

## Source Journals

- `docs/journal/2026-02-20-acp-package-and-bootstrap-module-migration.md`
- `docs/journal/2026-02-09-codex-acp-protocol-sync.md`
- `docs/journal/2026-02-13-acp-ui-fold-markdown-mobile.md`
- `docs/journal/2026-02-15-acp-tool-call-humanized-rendering.md`
- `docs/journal/2026-02-15-acp-conversation-stick-bottom-hardening.md`
- `docs/journal/2026-02-16-permission-history-agent-scope.md`
- `docs/journal/2026-02-17-acp-tool-call-group-fold-animation.md`
- `docs/journal/2026-02-20-web-tailwind-ui-phase8-acp-panel-debug-shell.md`
- `docs/journal/2026-02-20-web-tailwind-ui-phase9-acp-conversation-shell.md`
- `docs/journal/2026-03-08-sse-stale-running-agent-reconciliation.md`
- `docs/journal/2026-03-22-acp-provider-runtime-abstraction.md`
- `docs/journal/2026-03-24-codex-acp-native-skill-injection.md`
- `docs/journal/2026-03-30-codex-acp-apply-patch-deadlock.md`
- `docs/journal/2026-03-31-pretext-acp-conversation-virtualization.md`
- `docs/journal/2026-04-24-codex-custom-tool-output-hotfix.md`
- `docs/journal/2026-05-08-acp-permission-timeout-deny.md`
- `docs/journal/2026-05-09-acp-permission-tool-call-settlement.md`
- `docs/journal/2026-05-13-acp-permission-deny-drain.md`
- `docs/journal/2026-05-15-codex-acp-prompt-steering.md`
- `docs/journal/2026-06-03-acp-provider-metadata-allowlist.md`
- `docs/journal/2026-06-10-claude-acp-provider-support.md`
- `docs/journal/2026-06-11-generic-codex-acp-entrypoint.md`
- `docs/journal/2026-07-19-acp-ui-compaction-wave2.md`
- `docs/journal/2026-08-24-official-codex-0-149-1-acp-multimodal-standalone-ui.md`
- `docs/journal/2026-08-09-feature-docs-compaction-wave2-closeout.md`
