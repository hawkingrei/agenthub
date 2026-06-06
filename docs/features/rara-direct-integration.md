# Rara Direct Integration

## Problem

AgentHub can supervise long-lived ACP-backed coding agents, but Rara is a Linkerdog-owned runtime
rather than an opaque third-party provider binary. Treating Rara as an ACP subprocess would hide the
runtime control surface that Rara already defines for semantic input, structured output, memory,
skills, approvals, hooks, and diagnostics.

AgentHub therefore needs a direct Rara integration contract before implementation starts. The goal is
to let AgentHub run and supervise Rara as a first-class provider/runtime through Rara's app-server
control plane while preserving AgentHub's existing agent, Team, node, permission, and diagnostics
boundaries.

## Scope

- Local and remote AgentHub placement of a Rara runtime process.
- Rara app-server / runtime-control interaction as the only supported integration path.
- Session lifecycle, user input, follow-up, cancel, interrupt, approval, and output event mapping.
- Prompt-source, skill-source, memory, MCP, hook, and diagnostics boundaries that AgentHub may use
  after the first runtime slice.
- AgentHub UI/event persistence compatibility while Rara emits structured runtime events.

## Non-Goals

- Replacing Codex, Gemini, or Kimi ACP adapters.
- Using `rara acp` as a fallback path.
- Treating Rara's TUI, print mode, wire mode, or plain text output as the integration API.
- Importing Rara's SQLite files, `.rara/` memory, or internal thread store directly into AgentHub.
- Making AgentHub own Rara provider API keys or local model downloads.
- Embedding Rara as an in-process Rust library in the first slice.
- Promising every Rara runtime-control request family is implemented by AgentHub in phase 1.

## Architecture

### 1) Provider / Placement / Protocol Axes

Rara should use AgentHub's existing separation between provider identity and runtime placement:

- Provider adapter: `rara`
  - owns Rara-specific process startup, handshake, event translation, and diagnostics mapping
- Runtime placement:
  - starts local Rara under the selected AgentHub workspace in phase 1
  - later reuses AgentHub remote-node placement to start Rara on a remote Agent Node
- Protocol:
  - canonical and required: Rara app-server runtime-control over a structured byte stream

The provider identity must remain independent from placement. A remote Rara runtime should not require
a different provider adapter than a local Rara runtime.

### 2) Rara App-Server Boundary

Rara's direct integration boundary is its runtime-control protocol, based on:

- `RuntimeControlEnvelope`
- `RuntimeProvenance`
- `RuntimeControlRequest`
- structured runtime events from Rara's event bus

AgentHub launches Rara through the phase 1 app-server command and one child-process transport
contract:

```bash
rara app-server --protocol-version 1 --transport stdio-jsonl
```

The app-server stream is UTF-8 JSON Lines over stdio:

- one JSON object per line
- newline terminates each frame
- no pretty-printed multi-line JSON
- stdout is reserved for protocol frames
- stderr is reserved for human-readable diagnostics and must not be parsed as protocol state
- binary payloads are out of scope for phase 1

HTTP, sockets, and length-prefixed framing are future extensions, not phase 1 compatibility paths.

AgentHub must not simulate terminal keys, scrape TUI output, parse `rara print` text, or invoke
`rara acp` for runtime state. It should send semantic runtime-control requests and consume structured
events.

### 3) AgentHub Runtime Shape

AgentHub should add a Rara provider adapter behind the existing runtime/provider seams:

```text
AgentHub agent/session
  -> provider adapter: rara
  -> placement executor: local subprocess or remote node
  -> Rara app-server byte stream
  -> Rara runtime-control requests/events
```

AgentHub remains responsible for:

- agent record ownership and `agent_sessions.id`
- workspace selection and safe-path policy before launch
- Team actor identity, mailbox routing, task context, and skill injection policy
- event persistence, SSE/history replay, and browser rendering
- root/operator diagnostics surfaces

Rara remains responsible for:

- provider/model selection and provider credentials inside Rara config
- Rara thread/session continuity
- Rara sandbox and tool execution policy
- Rara memory under `<workspace>/.rara/` and Rara home/cache directories
- local model preparation and provider-specific model catalog behavior

### 4) Session Identity

AgentHub and Rara session identities must stay separate:

- `agent_sessions.id` is AgentHub's per-launch runtime/audit identifier.
- AgentHub persistent provider continuity should store Rara's thread/session identity separately.
- Rara's thread id must not replace AgentHub's agent id, actor id, Team member id, or session id.
- Force-new-session semantics clear Rara continuity intentionally; ordinary AgentHub restart should
  attempt to resume the stored Rara continuity id when the adapter reports it is reusable.

### 5) Event Translation

Rara app-server events should be normalized into AgentHub's existing agent event persistence and
conversation surfaces without pretending they are Codex-native or ACP-native events.

The first stable event families are:

- assistant text and deltas
- reasoning/thinking summaries when Rara exposes them
- tool lifecycle, progress, stdout/stderr, and result
- approval requests and approval settlements
- request-input prompts and answers
- plan/todo/context/memory updates
- warnings, errors, cancellation, and completion

Provider-native Rara metadata may be persisted only through an allowlist of safe ids, counters,
timestamps, event classes, and statuses. Prompt bodies, tool arguments, tool outputs, secrets, and
provider raw JSON must stay redacted from diagnostics metadata by default.

## Contracts

### 1) Startup And Handshake

- AgentHub starts Rara through a configured binary path, defaulting to `rara` on `PATH`.
- The required startup mode is an app-server/runtime-control mode, not `rara tui`, `rara print`,
  `rara wire`, or `rara acp`.
- The phase 1 command shape is:
  - argv[0]: configured Rara binary path
  - argv[1]: `app-server`
  - argv[2]: `--protocol-version`
  - argv[3]: `1`
  - argv[4]: `--transport`
  - argv[5]: `stdio-jsonl`
- AgentHub passes workspace, environment, and proxy policy through the placement layer.
- The first stdout frame must be a handshake event. AgentHub must not send runtime-control requests
  until this frame is accepted.
- Rara reports a handshake containing at least:
  - app-server protocol version
  - Rara version
  - transport id (`stdio-jsonl`)
  - supported request families
  - supported event families
  - shutdown capabilities
  - safe provider/model summary
  - current or resumable Rara thread/session identity when available
- AgentHub accepts the handshake only when:
  - the frame parses as valid JSON
  - the frame type is the app-server handshake
  - app-server protocol version is exactly `1`
  - transport id is exactly `stdio-jsonl`
  - all phase 1 required request and event families are present
  - required identity/version fields are non-empty
- Any missing required field, incompatible protocol version, incompatible transport id, unsupported
  request/event family, malformed JSON, or non-handshake first frame is a handshake rejection.
- If the app-server handshake is unsupported, AgentHub fails startup with an actionable
  `rara_app_server_unsupported` error instead of falling back to another Rara mode.
- Graceful shutdown is a semantic runtime-control request followed by child-process drain. Process
  kill is reserved for startup failure, transport loss, explicit force-stop, or graceful shutdown
  timeout.

### 2) Configuration

Rara must not be configured through `codex_acp.*` fields. AgentHub should introduce a Rara-specific
configuration surface, for example:

```toml
[rara]
binary = "rara"
transport = "stdio-jsonl"
default_provider = "deepseek"
default_model = "deepseek-chat"
```

The exact field names can evolve during implementation, but the boundary is stable:

- Rara provider settings are Rara settings, not Codex settings.
- AgentHub may choose default provider/model labels for startup, but Rara owns credential lookup and
  provider-specific config.
- AgentHub must not copy provider API keys from Rara config into AgentHub's database.

### 3) Input Control

AgentHub maps browser/API/Team input into Rara semantic requests:

| AgentHub action | Rara runtime-control request |
| --- | --- |
| first prompt for an idle session | `InputControlRequest::SubmitUserPrompt` |
| prompt while a turn is active | `InputControlRequest::SubmitFollowUp` |
| answer request-input prompt | `InputControlRequest::AnswerPendingInput` |
| answer plan approval | `InputControlRequest::AnswerPlanApproval` |
| answer shell/tool approval | `InputControlRequest::AnswerShellApproval` or `ApprovalControlRequest::AnswerPendingApproval` |
| explicit cancel | `SessionControlRequest::CancelCurrentTurn` |
| explicit interrupt/preempt | `SessionControlRequest::InterruptCurrentTurn` |

Follow-up input must preserve ordering and must not imply cancel or interrupt. AgentHub should expose
busy/rejected states if Rara rejects a queued follow-up.

### 4) Request Acceptance And Ack

Every AgentHub-submitted `RuntimeControlEnvelope.request_id` must receive a correlated Rara response
or runtime event before AgentHub treats the browser/API-side state transition as committed.

Required request lifecycle states are:

- `accepted`: Rara accepted and applied the request immediately
- `queued`: Rara accepted the request for later execution, preserving order
- `rejected`: Rara rejected the request with a stable reason code and safe message

Optional request lifecycle state:

- `completed`: terminal request outcome when Rara can report one separately from stream events

Contract details:

- `SubmitUserPrompt`, `SubmitFollowUp`, pending-input answers, plan approvals, shell/tool approvals,
  cancel, and interrupt all require request correlation.
- AgentHub may persist the user's attempted input before dispatch for auditability, but the visible
  committed state must reflect Rara's ack result.
- If Rara returns `rejected`, AgentHub should append a redacted visible status event and keep the
  original request pending/failed according to the existing AgentHub input semantics.
- If the app-server transport closes before ack, AgentHub must treat the request outcome as unknown
  and reconcile through event replay before retrying.
- Re-sending a request after an unknown outcome must reuse either the original `request_id` or an
  explicit `idempotency_key`; Rara must not apply the same accepted request twice.

### 5) Approval And Permission

- Rara owns local sandbox and tool approval semantics.
- AgentHub may render and answer Rara approval requests through its existing permission/review UI.
- Permission timeout should be converted into an explicit deny decision when Rara supplies a deny
  option; it must not silently kill or restart the Rara runtime.
- Team permission-review routing may be reused, but the requester must never review its own Rara
  approval request.

### 6) Prompt, Skills, Memory, MCP, And Hooks

AgentHub may provide Team/runtime context to Rara only through structured control-plane sources:

- prompt source registration for AgentHub/Team runtime context
- skill source registration for AgentHub-managed Team skills
- memory control requests for deliberate memory mutations or queries
- MCP control requests for status/refresh/reconnect
- hook declarations only after Rara and AgentHub agree on hook lifecycle policy

AgentHub must not concatenate raw Team prompt tails directly into Rara system prompts outside Rara's
source registration path. Rara's `<workspace>/.rara/` memory remains Rara-owned; AgentHub may archive
or reference summaries, but it must not treat Rara memory files as AgentHub's canonical Team memory.

### 7) Event Replay And Idempotency

Rara app-server events must carry enough identity for AgentHub to dedupe, replay, and diagnose
reconnect boundaries.

AgentHub should persist these provider-native identifiers alongside each normalized AgentHub event:

- Rara `event_id`
- Rara monotonic `sequence`
- Rara thread/session id
- AgentHub `agent_sessions.id`
- AgentHub agent id and, for Team members, actor/member ids

Replay contract:

- The tuple `(rara_thread_or_session_id, event_id)` is the primary dedupe key.
- `sequence` is the gap-detection cursor within one Rara thread/session stream.
- Reconnect should resume from the last persisted Rara sequence when Rara supports replay.
- If replay is unavailable or returns a gap, AgentHub must mark the stream as having a replay gap
  instead of silently rendering a partial timeline as complete.
- Duplicate tool results, deltas, approvals, and completion events must be ignored after dedupe.
- AgentHub should store the latest translated Rara sequence in the provider adapter diagnostics so
  `agenthub doctor agent-trace` can explain whether persistence, transport, or rendering is stale.

### 8) Diagnostics

`agenthub doctor agent-trace` and web debug surfaces should report a Rara provider adapter section
when the active provider is Rara:

- Rara process status and placement node
- app-server protocol version and handshake capabilities
- active Rara thread/session id when reported as safe metadata
- queued input count and active prompt state
- last Rara runtime event class and timestamp
- pending approvals/tool calls by safe id and status
- event translation cursor into AgentHub persistence
- latest Rara `event_id`, `sequence`, and replay-gap status when available

Diagnostics must stay read-only by default. Repair, restart, cancel, or interrupt actions require
explicit user/operator action.

### 9) Remote Nodes

Remote Rara execution should reuse AgentHub Agent Node placement:

- main AgentHub stores the shadow agent record and UI state
- selected Agent Node starts the Rara process in the node-local workspace
- Rara state and `.rara/` memory stay on the execution node
- AgentHub streams normalized events back through the existing remote-control/event path
- before remote launch, AgentHub must verify the selected node reports compatible Rara app-server
  capability:
  - Rara binary availability
  - app-server protocol version
  - transport id (`stdio-jsonl` for phase 1)
  - supported request/event families needed by the agent mode
  - safe workspace and environment readiness

Rara direct integration must not introduce a separate remote transport stack that bypasses AgentHub's
node registry, internal gRPC auth, or remote worktree policy.

If a remote node cannot report a compatible Rara app-server capability, AgentHub must fail before
creating or starting the remote runtime session.

## Validation Matrix

Phase 0 spec validation:

- `cargo fmt --check`
- `git diff --check`
- manual review against `linkerdog/rara`:
  - [README.md](https://github.com/linkerdog/rara/blob/main/README.md)
  - [runtime-control-plane.md](https://github.com/linkerdog/rara/blob/main/docs/features/runtime-control-plane.md)
  - [app-server-architecture.md](https://github.com/linkerdog/rara/blob/main/docs/features/app-server-architecture.md)
  - [runtime_control.rs](https://github.com/linkerdog/rara/blob/main/crates/rara-app-server/src/runtime_control.rs)

Phase 1 implementation validation:

- focused AgentHub config tests for Rara-specific config parsing and environment overrides
- provider adapter unit tests for app-server handshake and capability negotiation
- input mapping tests for submit, follow-up, pending answer, approval, cancel, and interrupt
- request ack tests for accepted, queued, rejected, unknown-before-ack, and idempotent retry
- event translation tests for assistant text, tool lifecycle, approval, request-input, error, and
  completion events
- replay/idempotency tests for duplicate event ids, sequence gaps, reconnect resume, and duplicate
  tool-result suppression
- redaction tests for Rara provider metadata in persisted events and `agenthub doctor agent-trace`
- remote-node capability preflight tests for incompatible protocol version, missing transport, and
  unsupported request families
- local smoke test that starts `rara` in app-server mode, sends one prompt, receives structured
  output, and shuts down cleanly
- remote-node smoke test after local mode is stable

## Operational Notes

- Use direct app-server integration only. Do not use `rara acp` for AgentHub-owned Rara integration.
- Keep Rara version/capability checks strict enough to fail fast when the app-server protocol drifts.
- Keep AgentHub and Rara memory stores separate. Share summaries and pointers, not raw database or
  memory-file ownership.
- Keep implementation slices small:
  1. Rara app-server command/handshake in Rara, plus AgentHub config contract
  2. local process launch + prompt/follow-up/cancel
  3. event translation + persistence/replay
  4. approvals + diagnostics
  5. Team skill/prompt-source injection
  6. remote-node placement

## Open Risks

- Rara currently has runtime-control types, but the stable app-server command/transport may need to
  be finalized in Rara before AgentHub can depend on it.
- Rara and AgentHub both have memory and skill concepts; careless sharing could create duplicated
  or conflicting context unless all cross-runtime data flows through structured source registration.
- AgentHub's existing ACP conversation UI may need neutral provider labels so Rara events do not
  appear as Codex-specific diagnostics.
- Rara local model preparation can be slow or resource-heavy; AgentHub startup and health checks
  should distinguish model bootstrap from runtime failure.
- Remote-node Rara placement may expose host capability differences that AgentHub does not yet
  inventory.

## Source Journals

- [2026-06-06-rara-app-server-phase1-contract.md](../journal/2026-06-06-rara-app-server-phase1-contract.md)
- The first implementation PR should add or update a dated journal that links back to this spec.

## External References

- [README.md](https://github.com/linkerdog/rara/blob/main/README.md)
- [runtime-control-plane.md](https://github.com/linkerdog/rara/blob/main/docs/features/runtime-control-plane.md)
- [app-server-architecture.md](https://github.com/linkerdog/rara/blob/main/docs/features/app-server-architecture.md)
- [runtime_control.rs](https://github.com/linkerdog/rara/blob/main/crates/rara-app-server/src/runtime_control.rs)
