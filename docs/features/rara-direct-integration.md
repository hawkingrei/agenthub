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

The dedicated configuration, bounded wire codec, connection lifecycle and managed
local launch/cleanup are implemented in `agenthub-config`, `agenthub-rara` and the
existing agent manager. The per-agent event database also provides durable control
receipts, event deduplication and contiguous replay cursors. Managed event consumption,
prompt/follow-up submission, fenced user answers, live permissions and turn cancellation
are integrated. Authorized receipt/cursor history remains available after exit, and startup
retires abandoned transport ownership. Loop admission remains an implementation gate tracked
in [the transition TODO](../todo.md).

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

### Loop Execution Boundary

Rara direct integration participates in the [agent loop runtime](agent-loop-runtime.md) as one
provider adapter behind the shared scheduler. This is a target alignment; the current integration
contract below remains authoritative until the loop lifecycle is implemented.

- One admitted activation delivers one configured role prompt through `SubmitUserPrompt`, or
  `SubmitFollowUp` when the adapter reports a reusable live turn. Rara-internal reasoning and tool
  rounds stay inside that activation.
- Activation identity is AgentHub-owned and distinct from both `agent_sessions.id` and Rara
  thread/session continuity. Rara continuity is provider continuity that a later activation may
  resume; losing it must not lose canonical task, IM, or outcome state.
- Handshake capabilities gate lifecycle claims. The scheduler must not record a durable wait for a
  Rara approval unless the handshake advertises approval persistence across process exit;
  otherwise Rara approvals keep the live-callback semantics of the runtime approval contract.
- Semantic guard results map to loop outcomes: `mismatch` records a no-actionable-work outcome
  with the guard's safe reason, and `needs_clarification` records a wait on the clarification
  reply. Neither is a crash, a cancellation, or a permission denial.
- A nested Rara subteam executes inside the outer member's activation. Internal subagents do not
  create AgentHub activations, Team members, or mailbox targets.
- Activation detail and `agenthub doctor agent-trace` join the selected local session to a
  bounded native runtime snapshot: committed sequence/gap, request kind/status and safe ACK
  identifiers. Finished and interrupted executions retain this evidence independently of
  process liveness. ACK sequence never substitutes for the committed event cursor. Missing
  ownership yields no runtime snapshot, and no other session is used as a fallback.

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

### 4) Rara Team Modes

Rara direct integration must support two different Team shapes without collapsing their identity
models:

- Local Rara agent team:
  - AgentHub starts one Rara app-server runtime in a local AgentHub workspace.
  - Rara may use a lightweight model as its internal team leader.
  - Rara may start one or more local subagents as internal workers under that Rara runtime.
  - AgentHub observes and supervises the outer Rara runtime as one AgentHub agent/session unless
    Rara explicitly exposes safe structured subagent telemetry.
- Remote AgentHub Team member:
  - AgentHub assigns the Rara-backed agent one canonical AgentHub Team identity and role.
  - The assigned AgentHub role may be `coordinator` or `worker`.
  - The Rara-backed agent may still create its own internal Rara subteam, including local or remote
    subagents, to complete the assigned work.
  - AgentHub mailbox routing, permission-review routing, task ownership, and Team conversation
    accountability remain bound to the outer AgentHub Team member identity, not to Rara's internal
    subagent ids.

This means Rara can be a Team runtime and can also contain a Rara-managed team. AgentHub must treat
those as nested layers:

```text
AgentHub Team member identity
  -> Rara app-server runtime
  -> optional Rara-managed local/remote subteam
```

The nested Rara subteam must not silently create additional AgentHub Team members, bypass AgentHub
mailbox delivery, or claim another AgentHub member's role.

### 5) Session Identity

AgentHub and Rara session identities must stay separate:

- `agent_sessions.id` is AgentHub's per-launch runtime/audit identifier.
- AgentHub persistent provider continuity should store Rara's thread/session identity separately.
- Rara's thread id must not replace AgentHub's agent id, actor id, Team member id, or session id.
- Force-new-session semantics clear Rara continuity intentionally; ordinary AgentHub restart should
  attempt to resume the stored Rara continuity id when the adapter reports it is reusable.

### 6) Event Translation

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
- Exact `request_methods`, rather than family names alone, establish operation
  support. The transport requires session create/query/cancel/interrupt, prompt
  and follow-up input, user/plan/shell answers, and semantic server shutdown.
  Role/source consumers must separately require the methods they use.
- Any missing required field, incompatible protocol version, incompatible transport id, unsupported
  request/event family, malformed JSON, or non-handshake first frame is a handshake rejection.
- If the app-server handshake is unsupported, AgentHub fails startup with an actionable
  `rara_app_server_unsupported` error instead of falling back to another Rara mode.
- Graceful shutdown is a semantic runtime-control request followed by child-process drain. Process
  kill is reserved for startup failure, transport loss, explicit force-stop, or graceful shutdown
  timeout.

The compatible protocol fixture is pinned to upstream commit
`6f489462251b73e1695bb22a59d2ece59ba26a21` in
[the independently validated prerequisite PR](https://github.com/linkerdog/rara/pull/885).
Package version `0.0.22` alone does not identify this protocol. The version 1 envelope
uses `type`/`payload`; the handshake carries `runtime_id`, `runtime_version`,
`request_methods`, family lists and explicit receipt/replay/approval lifetimes.
The tested build advertises runtime-only receipts and replay, and no persistent
approvals or session resume. Missing required capabilities fail startup visibly.

Frames contain at most 1,048,576 UTF-8 payload bytes, excluding LF or CRLF delimiters.
Blank, partial-EOF, malformed and oversized frames fail the transport. A cancelled
asynchronous read retains its partial frame for the next poll. All protocol errors
contain fixed categories without raw input or provider diagnostics.

The connection uses one ordered writer and a cancellation-safe reader. Its local
queues hold at most 8 commands, 8 encoded write packets and 32 output frames;
at most 32 requests await ACKs. A stalled event consumer terminates the transport
with an explicit incomplete-delivery error instead of silently dropping events.
Received frames must retain the negotiated runtime identity. ACKs and shutdown
completion require an issued request identity; a second handshake is rejected.

The connection sends each request identity once. Cancelling the caller after
queue admission does not retract or retry its operation. A request timeout closes
the connection with an unknown-outcome error. The local receipt identity bound
is the smaller of 4,096 and the peer's advertised limit, with one slot reserved for
shutdown. Durable receipt reconciliation and explicit replay policy belong to
the request/event mapping slice.

A shutdown ACK alone is not success. The same request must receive an accepted
ACK, then its matching `shutdown_complete`, then clean stdout EOF within the
shutdown deadline. Stdin stays open during drain; the child must not depend on its
EOF to initiate shutdown. This transport receipt does not prove process exit,
descendant cleanup, task completion or durable consumption of queued events.
Those remain the process supervisor's and event consumer's separate obligations.

Managed local startup selects this transport with agent command `rara` and empty
agent arguments. `[rara].binary` selects the actual executable. The existing local
executor supplies workspace, environment and proxy policy; the process supervisor
retains registration and cleanup ownership. Stderr is drained before handshake in
fixed-size chunks. Only a byte counter reaches diagnostics; diagnostic text cannot
grant readiness or enter the conversation as an ACP event.

Stopping one agent or the daemon first requests semantic shutdown and allows two
seconds for process exit after clean transport drain. The existing supervisor
then verifies process-group cleanup, with its signal/kill fallback on failure or
timeout. Startup failures and transport loss clean the same owned launch before
terminal state is recorded. Both exit watchers and live-session lookups require
semantic completion in addition to process success for this transport. The local
launch ID and negotiated runtime ID remain separate.

Managed startup creates one native session after the handshake. Its durable creation
ACK establishes stream ownership before initial events are consumed. Received events
commit history and cursor before broadcast; exit observation waits for that drain as
well as semantic transport completion. Managed text input maps idle submissions to prompts
and active-turn submissions to ordered follow-ups. A pending user question requires its
explicit runtime/session/waiting-turn fence. Image input is unsupported. Remote placement,
legacy Team sessions and legacy idle loops remain rejected. Reserved local loop activations
can use fresh native sessions through the shared launch and cleanup path. Live plan/shell
callbacks and fenced cancel/interrupt controls reuse the existing permission and control surfaces.

The native loop bootstrap requires `prompt_source.register` and `skill_source.register`
before creating a session. It pins the configured role entry and managed skills in the same
launch configuration used by other local adapters. The role entry and outer identity arrive
as a session-scoped user-layer prompt source; skills arrive as inline registrations. One
input starts the activation only after every registration has an accepted durable receipt
and its acknowledged event prefix has committed. A partial bootstrap is not retried in the
same native session. Source limits are checked before sending the first registration.

The `native-loop-v2` source contract is part of the configuration digest and entry version.
It keeps activation, local launch, native runtime and native session identities distinct;
native subagents receive no independent outer membership, mailbox or execution credentials.
An ordinary completed turn is insufficient to finish an activation: the existing structured
finish service owns that outcome. A terminal turn without an outcome becomes interrupted
only after existing supervised cleanup. A live input/approval wait keeps its callback owner;
canceling that wait may end the native turn through input-discarded alone.

The pinned build has no cross-process resume, durable approval recovery, controlled MCP source
registration, or semantic-guard event contract. Resume policies and configured native MCP/App
bindings therefore fail preflight. Loop launches disable ambient extension discovery and native
memory facilities. They never replace missing controlled sources with ambient configuration.
Card/task source binding, stable task memory prefixes and activation trace enrichment are
implemented. Controlled tool sources and semantic outcome adaptation remain unfinished.

### 2) Configuration

Rara must not be configured through `codex_acp.*` fields. AgentHub should introduce a Rara-specific
configuration surface, for example:

```toml
[rara]
binary = "rara"
transport = "stdio-jsonl"
default_provider = "deepseek"
default_model = "deepseek-chat"
startup_timeout_seconds = 120
shutdown_timeout_seconds = 30
```

The exact field names can evolve during implementation, but the boundary is stable:

- Rara provider settings are Rara settings, not Codex settings.
- AgentHub may choose default provider/model labels for startup, but Rara owns credential lookup and
  provider-specific config.
- AgentHub must not copy provider API keys from Rara config into AgentHub's database.

The binary defaults to `rara`; provider/model defaults are optional and leave
runtime-owned credential resolution intact. Only `stdio-jsonl` is accepted.
Timeouts must be1-600 seconds. Explicit overrides are
`AGENTHUB_RARA_BINARY`, `AGENTHUB_RARA_PROVIDER`, `AGENTHUB_RARA_MODEL`,
`AGENTHUB_RARA_STARTUP_TIMEOUT_SECONDS` and
`AGENTHUB_RARA_SHUTDOWN_TIMEOUT_SECONDS`. Unknown fields, including credential
fields, reject configuration. Startup argv uses the fixed protocol prefix and
one argument per value; model/provider labels cannot inject permission flags.
An agent's explicit `runtime_model` overrides the configured default model;
thinking-level overrides remain unsupported. The overall start admission deadline
is at least the configured handshake timeout plus five seconds for local setup.

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

The durable receipt boundary is the owning agent's event database. A request records
its method and target before dispatch, then obtains a single process-local send permit
only after committing send intent with SQLite `synchronous = FULL`. Preparing the same
identity or obtaining a second send permit is rejected. Receipt preparation is bounded
to 4,096 identities per runtime; the negotiated transport limit can be lower.

Receipts distinguish `prepared`, `sent`, `accepted`, `queued`, `rejected`,
`outcome_unknown` and `not_sent`. Closing an owned runtime retires unsent preparations
and marks unresolved sends unknown. A correlated late ACK can resolve uncertainty but
does not authorize another send. Creation ACK and native stream ownership commit
together; ACK sequence information never advances the persisted event cursor. Cancel
and interrupt ACKs must name the fenced turn; pending-input answers may start a new turn.
Receipt metadata contains safe identifiers, method/status, timestamps and an allowlisted
rejection code, without request bodies or provider rejection prose. No control-request
outcome authorizes an automatic replacement send.

Managed user input atomically persists the attempted conversation message and prepared
receipt under the caller's message ID before sending. A daemon-owned task finishes receipt
persistence even if the HTTP caller disconnects. Reusing the ID cannot create another
attempt or send. The conversation separately displays sending, accepted, queued, rejected,
not-sent or unknown delivery; acceptance is not execution completion. Receipt updates match
local session, runtime, native session and request ID, including out-of-order history pages.

The input API accepts an optional `native_input` object with `runtime_id`, `session_id` and
`turn_id`. Question cards carry their original target through both web workbenches. A stale
native answer is never retargeted to a replacement local session, and malformed native cards
cannot fall back to ordinary text input. Untargeted text cannot answer a pending question
or permission. Existing ACP callers retain their input format and session-retry behavior.

The typed control mapper validates target, turn and encoded size before dispatch.
Native shell rejection is explicitly represented as `Deny`, serialized to the pinned
protocol's `suggestion` decision, which rejects execution and resumes reasoning.

Event projection uses the outer owned session, not optional provenance, and computes
a SHA-256 fingerprint over recursively sorted JSON object keys. Assistant deltas retain
contiguous message identities. Tool output follows the original open call across an
approval answer's new turn; later reuse of a completed call ID creates a new card.
After an owned shell approval is granted, the native repeated start event updates the
original card once in the answer turn. Other duplicate starts remain conflicts. A native
plan answer completes its interaction card without requiring a tool-result event.
Terminal or discarded turns fail unfinished tool cards and retire their identities;
an old turn's cleanup cannot retire calls owned by its successor.
Question cards carry runtime, native session and waiting turn for reply validation.
An approval notice alone never creates a live callback. Projection state is installed
only after its event transaction commits; duplicates and failed transactions cannot
advance chunk or tool state. Diagnostic/source events expose allowlisted identifiers,
counts and statuses, while conversation bodies remain attributed history content.

### 5) Approval And Permission

- Rara owns local sandbox and tool approval semantics.
- AgentHub may render and answer Rara approval requests through its existing permission/review UI.
- Permission timeout should be converted into an explicit deny decision when Rara supplies a deny
  option; it must not silently kill or restart the Rara runtime.
- Team permission-review routing may be reused, but the requester must never review its own Rara
  approval request.

Only a committed pending plan or shell input creates a live permission callback. It retains
the original tool card and runtime/session/waiting-turn ownership. Explicit option IDs map
to native decisions; unknown choices and cancellation cannot grant execution. Timeout sends
one explicit denial while the waiting turn remains owned. Superseded inputs, turn cancellation
and transport loss expire the callback without answering a replacement turn.

The operator's selected choice and the native control ACK are recorded separately. Rejected
or uncertain answers never become implicit approval or an automatic retry. Cancel/interrupt
controls capture their target once and require a matching accepted ACK; they cannot stop a
successor turn. A transport failure retires the owned runtime through existing supervision.

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

Task-scoped memory updates may derive their routing prefix from the canonical Team task expression.
The prefix must be stable for the lifetime of that task:

- derive the prefix from the task id plus normalized task title/summary, not from transient chat text
- persist the derived prefix with the task or Rara thread continuity before writing memory
- reuse the same prefix for follow-ups, clarifications, retries, and nested Rara subteam work for
  that task
- create a new prefix only when AgentHub creates a new canonical task or explicitly retitles/rekeys
  the task

This lets Rara tune memory around the task wording while avoiding prefix drift across turns.

Native loop startup pins the member's discovery Card and canonical task title/`context.summary`
from its non-revoked activation sources before spawning the provider. Other task context fields
and transient chat are excluded. The launch digest includes this source snapshot. One bounded
prompt source carries each task expression, alongside the outer role source; more than 31 distinct
tasks fails startup without silently dropping accepted work.

The control database stores `task-memory-v1` prefixes derived from Team ID, task ID and normalized
initial title/summary before source registration. Follow-ups, fresh sessions, clarification and
ordinary task wording updates reuse the stored prefix; new task IDs derive distinct prefixes.
Deleting a task deletes its routing record. Prefix creation requires the current activation fence,
membership and a non-revoked reference to that task; the prefix grants no execution authority.

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

- The tuple `(runtime_id, owning_session_id, event_id)` is the primary dedupe key
  for the pinned runtime-only stream. The outer event envelope supplies the owned
  session independently of event provenance, which may have no session ID.
- `sequence` is the gap-detection cursor within one Rara thread/session stream.
- Reconnect should resume from the last persisted Rara sequence when Rara supports replay.
- If replay is unavailable or returns a gap, AgentHub must mark the stream as having a replay gap
  instead of silently rendering a partial timeline as complete.
- Duplicate tool results, deltas, approvals, and completion events must be ignored after dedupe.
- AgentHub should store the latest translated Rara sequence in the provider adapter diagnostics so
  `agenthub doctor agent-trace` can explain whether persistence, transport, or rendering is stale.

Event persistence commits the event identity/digest, zero or more normalized history
rows, their native-event associations and the next contiguous cursor in one transaction.
An identical replay emits no history rows. Reusing an ID or sequence with different
content is an error. An out-of-order event returns the missing position without writing
history or advancing the cursor; the adapter must bound its pending events and recover
through the negotiated replay mechanism.

A replay response that cannot supply the missing prefix, or whose latest sequence is
behind the committed cursor, records an explicit incomplete-stream boundary. It cannot
skip or rewind the cursor. History retention removes transcript associations together
with their history rows but preserves deduplication receipts and cursors, so replay
cannot resurrect expired transcript content. Local launch, runtime and native session
ownership remain distinct; unsolicited events cannot allocate their own binding.
Reopening this database for a live runtime is not evidence of cross-process native
session resume or durable approval support.

The managed consumer buffers at most 256 out-of-order events and 8 MiB. It requests
replay from the committed contiguous cursor without blocking output consumption on
the ACK. A replay must finish within 30 seconds; absent replay support, overflow,
identity conflicts and unavailable history fail visibly. Only persisted events update
live phase/pending-input state and presentation state. The pinned stdio protocol does
not reconnect across process lifetimes; reopening a cursor alone cannot rebuild live
projection state or resurrect a pending approval.

An accepted ACK may precede delivery of its referenced events. Before choosing the next
prompt/follow-up or turn control, the adapter waits for that cursor to commit, bounded by
the replay timeout. This wait never advances event persistence from ACK metadata alone.

Startup recovery runs under the daemon's exclusive instance lock before new work is admitted.
It closes earlier local stdio ownership, settles prepared requests as `not_sent` and unresolved
sends as `outcome_unknown`, and expires live permission callbacks. Recorded ACKs remain intact.
Native sessions waiting on approval are included even when their status is not `running`.
This is transport retirement, not evidence that detached processes stopped or tasks finished;
durable execution reservations retain their separate cleanup fence.

### 8) Diagnostics

`GET /api/agents/{id}/sessions/{session_id}/runtime` requires `runtime:inspect` and verifies
the local agent/session association before reading its event database. This release-visible
endpoint returns the owned runtime, closed state, native stream cursors/gaps and safe typed
request receipts after process exit. Unknown, foreign and remote sessions return not found.
It never creates runtime ownership or returns raw input, provider envelopes or rejection prose.
Receipts use descending request-ID pagination with `before_request_id`, a default limit of 50
and a maximum of 100. Stream summaries are bounded to 100 with explicit truncation metadata.
The response is one database read snapshot; ACK updates do not change receipt page ordering.

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

### 9) Team Role And Agent Card Guard

When AgentHub starts Rara as a Team member, startup context must include the outer Team identity:

- AgentHub team id
- AgentHub member id / actor id
- assigned AgentHub Team role (`coordinator` or `worker`)
- the member name and safe discovery Card fields: description, role, skill references and
  capability tags, together with the explicit outer collaboration boundary
- canonical task expression when the Team work is task-backed, including task id, title, summary, and
  the stable memory prefix if one already exists

Rara may use a lightweight semantic judge before acting on remote Team conversation or mailbox
context. The judge decides whether the incoming conversation/task is compatible with the assigned
AgentHub member's agent card and current role.

The semantic guard has three stable outcomes:

- `compatible`: proceed normally
- `mismatch`: do not execute the request; return a safe status event explaining that the request does
  not match the assigned agent card or role
- `needs_clarification`: ask AgentHub or the Team conversation for clarification before executing

Guardrails:

- The lite judge is advisory control flow inside the Rara runtime; it must not mutate AgentHub's
  canonical Team role, task owner, or member card directly.
- A `mismatch` result must not be treated as a runtime crash, cancel, or permission denial.
- If Rara proposes an agent-card update, it must use AgentHub's profile patch proposal flow rather
  than editing the Team card out of band.
- For local Rara agent teams that are not attached to an AgentHub Team, the same guard may validate
  against Rara's own internal agent cards, but AgentHub does not interpret those internal cards as
  AgentHub Team membership.

### 10) Remote Nodes

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
- Team-mode startup tests for local Rara team context and remote AgentHub Team member context
- semantic guard translation tests for `compatible`, `mismatch`, and `needs_clarification`
- nested subteam identity tests that verify Rara internal subagent ids do not become AgentHub Team
  member ids or mailbox targets
- task-scoped memory prefix tests that verify follow-ups and retries reuse the same prefix while new
  canonical tasks receive distinct prefixes
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
  6. local Rara team mode with lite leader and internal workers
  7. remote AgentHub Team member mode with role/card guard
  8. remote-node placement

## Open Risks

- Deployment must supply the tested upstream build or an independently validated
  compatible implementation. Shared enums or a matching package version do not
  establish command, capability or cleanup support.
- Rara and AgentHub both have memory and skill concepts; careless sharing could create duplicated
  or conflicting context unless all cross-runtime data flows through structured source registration.
- AgentHub's existing ACP conversation UI may need neutral provider labels so Rara events do not
  appear as Codex-specific diagnostics.
- Rara local model preparation can be slow or resource-heavy; AgentHub startup and health checks
  should distinguish model bootstrap from runtime failure.
- Remote-node Rara placement may expose host capability differences that AgentHub does not yet
  inventory.
- Nested Team semantics can become confusing if Rara internal subagents are displayed as AgentHub
  Team members without an explicit product decision.
- The lite semantic guard needs stable safe input fields and explainable outcomes; otherwise it could
  reject valid Team work or hide role/card drift behind model judgment.

## Source Journals

- [2026-09-18: Direct runtime loop activation](../journal/2026-09-18-native-loop-activation.md)
- [2026-09-18: Direct runtime event storage](../journal/2026-09-18-runtime-event-storage.md)
- [2026-09-18: Direct runtime transport](../journal/2026-09-18-rara-local-transport.md)

- [2026-06-06-rara-app-server-phase1-contract.md](../journal/2026-06-06-rara-app-server-phase1-contract.md)
- [2026-06-08-rara-team-modes-requirements.md](../journal/2026-06-08-rara-team-modes-requirements.md)
- The first implementation PR should add or update a dated journal that links back to this spec.

## External References

- [README.md](https://github.com/linkerdog/rara/blob/main/README.md)
- [runtime-control-plane.md](https://github.com/linkerdog/rara/blob/main/docs/features/runtime-control-plane.md)
- [app-server-architecture.md](https://github.com/linkerdog/rara/blob/main/docs/features/app-server-architecture.md)
- [runtime_control.rs](https://github.com/linkerdog/rara/blob/main/crates/rara-app-server/src/runtime_control.rs)
